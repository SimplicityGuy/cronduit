use std::path::Path;

/// Validate a config file. Plan 03 fills this in with the full
/// parse_and_validate pipeline and GCC-style error reporter.
pub async fn execute(config: &Path) -> anyhow::Result<i32> {
    eprintln!(
        "cronduit check: not yet implemented (config={}); Plan 03 wires this up.",
        config.display()
    );
    Ok(2)
}
