# Temporary SQLx SQLite security backport

`sqlx-sqlite-0.9.0/` is the published crates.io archive, SHA-256
`488e99c397a62007e4229aec669a179816339afc6d2620ca6fa420dbee2e982c`.
Download: https://static.crates.io/crates/sqlx-sqlite/sqlx-sqlite-0.9.0.crate
Published source revision: `003b698e99e024f3621b8043a2426fde5b741171`.

The only code/build change is widening the `libsqlite3-sys` upper bound in
`Cargo.toml` and `Cargo.toml.orig` from `<0.38.0` to `<0.40.0`, backporting
https://github.com/transact-rs/sqlx/commit/10462fa4414c6c7f1ed1d39bb31678e3b726878c.
That upstream commit changes its then-current `<0.39.0` bound to `<0.40.0`.
Every Rust source file is unchanged. The archive's LICENSE-MIT/LICENSE-APACHE
files contain relative path placeholders; these are materialized with the full
upstream license texts at the published source revision above.

The root `[patch.crates-io]` applies this bound to SQLx and its macros together.
The root lockfile selects `libsqlite3-sys 0.39.1` (bundled SQLite 3.54.1), fixing
[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459).
The vendor's original library lockfile is preserved for source provenance;
Cargo builds this project using the root lockfile.

Remove this directory and the root patch when a published SQLx SQLite release
permits a patched `libsqlite3-sys >=0.39.0`; update the root lockfile and rerun
`just deny`, `just check`, `just nextest`, and `just schema-diff`.
