# Phase 12.1 — GHCR Tag Hygiene _(INSERTED)_

Status: **not yet planned**. Run `/gsd-plan-phase 12.1` to decompose.

See `.planning/ROADMAP.md` § Phase 12.1 for goal, requirements (OPS-09, OPS-10),
and key design decisions.

Trigger for insertion: Phase 12 post-push GHCR verification (2026-04-19) surfaced
a pre-existing `:latest` divergence — `:latest` points at an older digest while
`:1`, `:1.0`, `:1.0.1` all agree on the v1.0.1 retag digest. Alongside fixing
that, we're adding a `:main` floating tag for operators who want bleeding-edge
main builds.

This phase must land before Phase 13 cuts `v1.1.0-rc.2` so rc.2 ships into a
healthy tag ecosystem.
