use crate::config;
use std::path::Path;

/// Validate a config file. Calls the shared `parse_and_validate` pipeline
/// and prints GCC-style `path:line:col: error: <msg>` on stderr.
///
/// Never touches the database (FOUND-03). Never prints secret VALUES --
/// only the variable NAMES that are missing (D-22, Pitfall 18).
pub async fn execute(config_path: &Path) -> anyhow::Result<i32> {
    match config::parse_and_validate(config_path) {
        Ok(_parsed) => {
            eprintln!("ok: {}", config_path.display());
            Ok(0)
        }
        Err(errors) => {
            for e in &errors {
                eprintln!("{e}"); // uses the GCC-style Display impl from src/config/errors.rs
            }
            eprintln!();
            eprintln!("{} error(s)", errors.len());
            Ok(1)
        }
    }
}
