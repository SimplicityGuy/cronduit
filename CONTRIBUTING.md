<!-- generated-by: gsd-doc-writer -->
# Contributing to Cronduit

Thanks for your interest in Cronduit — a self-hosted, Docker-native cron scheduler with a web UI, shipped as a single Rust binary. This document covers the contribution workflow, the local gates you must pass before opening a pull request, and the conventions the project enforces.

Read it before you open a PR; it is short by design.

## Development setup

This file does not duplicate setup instructions. Start here:

- **[docs/QUICKSTART.md](docs/QUICKSTART.md)** — prerequisites and getting a Cronduit instance running.
- **[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)** — local dev loop, the `just` task runner, and editing the UI.
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — how the scheduler, executor, persistence, and web layers fit together.
- **[docs/SPEC.md](docs/SPEC.md)** — the behavioral specification: what Cronduit must do and why.

Toolchain: stable Rust, edition 2024, `rust-version = 1.94.1` (see `Cargo.toml`). The repo uses [`just`](https://github.com/casey/just) as the single source of truth for every build, lint, test, and DB task — `just --list` shows the full menu. CI invokes the same `just` recipes you run locally, so `just ci` predicts the CI exit code.

## Contribution workflow

All changes land via a pull request from a feature branch. There are **no direct commits or pushes to `main`** — `main` is protected and only advances through merged PRs.

1. Fork the repo (or create a branch if you have push access).
2. Create a feature branch off `main`. Use a short, kebab-case, type-prefixed name, matching the project's existing branch style:

   ```bash
   git switch -c fix/dashboard-tag-filter
   git switch -c feat/postgres-pool-tuning
   git switch -c docs/contributing-guide
   ```

3. Make your change, keeping commits focused.
4. Run the full local gate (`just ci`) and make it green.
5. Push the branch and open a PR against `main`.

## Commit and PR conventions

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). The type prefix drives the changelog, so use a real one:

- `feat:` — a new capability
- `fix:` — a bug fix
- `docs:` — documentation only
- `test:` — tests only
- `chore:` — tooling, deps, housekeeping

A scope in parentheses is encouraged to point at the affected area, e.g. `fix(web): widen name-filter hx-include` or `feat(webhooks): HMAC signing`. Keep the subject line in the imperative mood and under ~72 characters.

PRs are squash-merged, so the **PR title becomes the squash commit subject** — write it as a Conventional Commit. GitHub appends the `(#NN)` PR-number suffix automatically; you do not add it.

In your PR description:

- Explain what changed and why.
- Link any related issue.
- Note user-visible changes (config, UI, metrics, CLI flags).
- Confirm `just ci` passes locally.

## Quality gates

Every PR must pass CI before it can merge. CI runs the same ordered chain as `just ci`:

```bash
just ci   # fmt-check → clippy → openssl-check → nextest → schema-diff → image
```

Run individual gates while iterating:

| Gate | Recipe | What it enforces |
|------|--------|------------------|
| Format | `just fmt-check` | `cargo fmt --all -- --check`; fix with `just fmt` |
| Lint | `just clippy` | `cargo clippy --all-targets --all-features -- -D warnings` (warnings are errors) |
| TLS hygiene | `just openssl-check` | `openssl-sys` must be absent from the dep tree across native + amd64-musl + arm64-musl (rustls-only) |
| Tests | `just nextest` | `cargo nextest run --all-features --profile ci` |
| Schema parity | `just schema-diff` | SQLite and Postgres schemas stay in lockstep |
| Image build | `just image` | the multi-arch Docker image still builds |
| Supply chain | `just deny` | `cargo deny check advisories licenses bans` (blocking) |

Additional CI gates worth knowing about:

- **`just grep-no-percentile-cont`** — fails if any file under `src/` uses SQL-native percentile functions; p50/p95 is computed in Rust for SQLite/Postgres parity.
- **Test matrix** — `amd64` and `arm64`; both SQLite and Postgres backends are exercised in every cell.
- **Compose smoke** — boots the example stack and confirms the four quickstart jobs reach `success`.
- **Webhook interop** — verifies the Python/Go/Node example receivers against the locked Standard Webhooks v1 fixture.

Fast inner-loop helpers: `just test-unit` (lib tests only) and `just dev` / `just dev-ui` (run the daemon against `examples/cronduit.toml`). See [docs/TESTING.md](docs/TESTING.md) for the full test layering.

If you change SQL, regenerate the offline query cache before committing: `just sqlx-prepare`.

## Project conventions

A few rules are non-negotiable and enforced in review:

- **Diagrams are mermaid.** Every diagram in any artifact — README, docs, PR descriptions, code comments — must be a fenced ```mermaid``` block. No ASCII-art diagrams.
- **The tech stack is locked.** Rust + `bollard` (Docker API, no CLI shelling) + `sqlx` (SQLite default, Postgres optional) + `askama` / `askama_web` for server-rendered HTML + `croner` for cron parsing + TOML for config + rustls everywhere. The full rationale lives in `CLAUDE.md`; do not introduce alternatives (no `serde-yaml`, no `askama_axum`, no OpenSSL, no SPA frameworks) without first opening an issue to discuss.
- **Security posture.** No plaintext secrets in config — interpolate from env and wrap in a `SecretString`. Default bind stays `127.0.0.1:8080`. The threat model is documented in [THREAT_MODEL.md](THREAT_MODEL.md).
- **`just` recipes are the interface.** Build, test, and lint through `just`; CI calls the same recipes, so a local pass means a CI pass.

## Reporting issues

Open issues on the [GitHub tracker](https://github.com/SimplicityGuy/cronduit/issues). For bugs, include:

- Cronduit version (or commit) and how you run it (binary vs. Docker image).
- Database backend (SQLite or Postgres) and host OS / architecture.
- The relevant slice of your `cronduit.toml` (with secrets redacted).
- Steps to reproduce, what you expected, and what actually happened.
- Relevant log output (run with `RUST_LOG=debug,cronduit=trace` for detail).

For feature requests, describe the use case and how it fits the locked stack and security posture above.

## License

Cronduit is released under the [MIT License](LICENSE). By contributing, you agree that your contributions are licensed under the same terms.
