---
status: complete
completed: 2026-04-22
---

# Completion marker

This quick task is **complete**. It fixed the `get_dashboard_jobs` Postgres `enabled` BIGINT comparison (dashboard was silently broken on the Postgres backend) and added a testcontainers-Postgres regression guard.

Full details: [`260421-nn3-SUMMARY.md`](./260421-nn3-SUMMARY.md).

_(This bare `SUMMARY.md` exists so the GSD `audit-open` quick-task scan — which keys on `status: complete` — recognizes the task as done. The detailed record lives in the prefixed file above.)_
