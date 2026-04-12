//! Integration test: config file modification triggers reload via file watcher.
//!
//! Tests RELOAD-03.
//! Verifies that spawn_file_watcher sends SchedulerCmd::Reload after debounce.

use std::time::Duration;
use tempfile::TempDir;

use cronduit::scheduler::cmd::SchedulerCmd;
use cronduit::scheduler::reload::spawn_file_watcher;

#[tokio::test]
async fn file_change_triggers_reload_command() {
    let tmp_dir = TempDir::new().expect("create temp dir");
    let config_path = tmp_dir.path().join("cronduit.toml");

    // Write initial config
    let initial_config = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "job-a"
schedule = "*/5 * * * *"
command = "echo a"
"#;
    std::fs::write(&config_path, initial_config).expect("write initial config");

    // Create channel to receive scheduler commands
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);

    // Spawn the file watcher
    spawn_file_watcher(config_path.clone(), cmd_tx).expect("spawn file watcher");

    // Wait for watcher to initialize
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Modify the config file
    let modified_config = r#"
[server]
timezone = "UTC"

[[jobs]]
name = "job-a"
schedule = "*/5 * * * *"
command = "echo a"

[[jobs]]
name = "job-b"
schedule = "0 * * * *"
command = "echo b"
"#;
    std::fs::write(&config_path, modified_config).expect("write modified config");

    // Wait for debounce (500ms) + margin
    let received = tokio::time::timeout(Duration::from_secs(3), cmd_rx.recv()).await;
    assert!(received.is_ok(), "should receive a command within timeout");
    let cmd = received.unwrap().expect("channel should not be closed");
    assert!(
        matches!(cmd, SchedulerCmd::Reload { .. }),
        "expected SchedulerCmd::Reload, got: {cmd:?}"
    );

    // Verify no duplicate events after debounce (wait a bit and check)
    let second = tokio::time::timeout(Duration::from_millis(800), cmd_rx.recv()).await;
    assert!(
        second.is_err(),
        "should NOT receive a second Reload after debounce (debounce should coalesce)"
    );
}

#[tokio::test]
async fn rapid_edits_coalesced_by_debounce() {
    let tmp_dir = TempDir::new().expect("create temp dir");
    let config_path = tmp_dir.path().join("cronduit.toml");

    // Write initial config
    std::fs::write(&config_path, "[server]\ntimezone = \"UTC\"\n").expect("write config");

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SchedulerCmd>(16);
    spawn_file_watcher(config_path.clone(), cmd_tx).expect("spawn file watcher");

    // Wait for watcher to initialize
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Rapid-fire edits within the debounce window
    for i in 0..5 {
        let content = format!(
            "[server]\ntimezone = \"UTC\"\n\n[[jobs]]\nname = \"job-{i}\"\nschedule = \"*/5 * * * *\"\ncommand = \"echo {i}\"\n"
        );
        std::fs::write(&config_path, &content).expect("write rapid edit");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Wait for debounce to settle
    let received = tokio::time::timeout(Duration::from_secs(3), cmd_rx.recv()).await;
    assert!(
        received.is_ok(),
        "should receive at least one Reload command"
    );
    let cmd = received.unwrap().expect("channel not closed");
    assert!(matches!(cmd, SchedulerCmd::Reload { .. }));

    // There may be at most one more Reload (depending on timing), but not 5
    // Drain any remaining and count
    let mut extra_count = 0;
    while let Ok(Some(_)) =
        tokio::time::timeout(Duration::from_millis(800), cmd_rx.recv()).await
    {
        extra_count += 1;
    }
    assert!(
        extra_count <= 1,
        "debounce should coalesce rapid edits; got {} extra reload commands",
        extra_count
    );
}
