//! End-to-end tests for the [defaults] merge fix (issue #20).
//!
//! These tests run the full parse_and_validate pipeline against a TOML
//! fixture written to a NamedTempFile, exercising parse + interpolate +
//! merge + validate. They are deliberately integration tests rather than
//! unit tests because the goal is to catch any future refactor of
//! parse_and_validate that drops the apply_defaults step.

use cronduit::config::ConfigError;
use cronduit::config::hash::compute_config_hash;
use cronduit::config::parse_and_validate;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

const SERVER_HEADER: &str = r#"
[server]
bind = "127.0.0.1:8080"
timezone = "UTC"
"#;

fn write_toml(contents: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("tempfile");
    f.write_all(contents.as_bytes()).expect("write");
    f.flush().expect("flush");
    f
}

#[test]
fn defaults_image_passes_validation_and_classifies_as_docker() {
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"

[[jobs]]
name = "dockerjob"
schedule = "*/5 * * * *"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.image.as_deref(), Some("alpine:latest"));
    // Inline classifier -- sync::job_type is pub(crate); replicate here.
    let jt = if job.command.is_some() {
        "command"
    } else if job.script.is_some() {
        "script"
    } else if job.image.is_some() {
        "docker"
    } else {
        "unknown"
    };
    assert_eq!(jt, "docker");
}

#[test]
fn defaults_network_container_vpn_preserved() {
    // MARQUEE FEATURE regression: container:vpn must flow through the merge.
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"
network = "container:vpn"

[[jobs]]
name = "vpn-job"
schedule = "*/5 * * * *"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.network.as_deref(), Some("container:vpn"));
}

#[test]
fn defaults_volumes_preserved_and_cloned() {
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"
volumes = ["/host/a:/a", "/host/b:/b"]

[[jobs]]
name = "vol-job"
schedule = "*/5 * * * *"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(
        job.volumes,
        Some(vec!["/host/a:/a".to_string(), "/host/b:/b".to_string()])
    );
}

#[test]
fn defaults_timeout_preserved() {
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"
timeout = "5m"

[[jobs]]
name = "timeout-job"
schedule = "*/5 * * * *"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.timeout, Some(Duration::from_secs(300)));
}

#[test]
fn defaults_delete_preserved() {
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"
delete = true

[[jobs]]
name = "delete-job"
schedule = "*/5 * * * *"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.delete, Some(true));
}

#[test]
fn job_override_wins_image() {
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"

[[jobs]]
name = "override-image"
schedule = "*/5 * * * *"
image = "nginx:1.25"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.image.as_deref(), Some("nginx:1.25"));
}

#[test]
fn job_override_wins_network() {
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"
network = "bridge"

[[jobs]]
name = "override-network"
schedule = "*/5 * * * *"
network = "container:vpn"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.network.as_deref(), Some("container:vpn"));
}

#[test]
fn use_defaults_false_disables_merge() {
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"

[[jobs]]
name = "opt-out"
schedule = "*/5 * * * *"
command = "echo hi"
use_defaults = false
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.image, None);
    assert_eq!(job.command.as_deref(), Some("echo hi"));
}

#[test]
fn docker_job_with_no_image_anywhere_still_fails() {
    // No [defaults].image, no per-job image, no command/script.
    // Validation must reject it AND the error must mention [defaults].
    let toml = format!(
        r#"{SERVER_HEADER}
[[jobs]]
name = "broken-docker"
schedule = "*/5 * * * *"
"#
    );
    let f = write_toml(&toml);
    let errors = parse_and_validate(f.path()).expect_err("must fail validation");
    let job_error = errors
        .iter()
        .find(|e| e.message.contains("broken-docker"))
        .expect("error referencing the broken job must be present");
    assert!(
        job_error.message.contains("[defaults]"),
        "error message must point at [defaults] as a valid source of image, got: {}",
        job_error.message
    );
}

#[test]
fn defaults_section_absent_is_legal() {
    let toml = format!(
        r#"{SERVER_HEADER}
[[jobs]]
name = "plain-cmd"
schedule = "*/5 * * * *"
command = "echo hi"
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(job.image, None);
    assert_eq!(job.network, None);
    assert_eq!(job.volumes, None);
    assert_eq!(job.delete, None);
    assert_eq!(job.command.as_deref(), Some("echo hi"));
}

#[test]
fn hash_stable_across_defaults_representations() {
    // For each defaults-eligible field, the field set on the job
    // directly must hash identically to the same field set in [defaults]
    // and merged in via parse_and_validate's apply_defaults call. This
    // is the end-to-end guard that future refactors of parse_and_validate
    // do not silently drop fields during the rebuild.

    fn parse(toml: &str) -> cronduit::config::ParsedConfig {
        let f = write_toml(toml);
        parse_and_validate(f.path()).expect("must parse")
    }

    // image
    {
        let a = parse(&format!(
            r#"{SERVER_HEADER}
[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
"#
        ));
        let b = parse(&format!(
            r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"

[[jobs]]
name = "j"
schedule = "*/5 * * * *"
"#
        ));
        assert_eq!(
            compute_config_hash(&a.config.jobs[0]),
            compute_config_hash(&b.config.jobs[0]),
            "image: hash must be stable across job-vs-defaults"
        );
    }

    // network
    {
        let a = parse(&format!(
            r#"{SERVER_HEADER}
[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
network = "container:vpn"
"#
        ));
        let b = parse(&format!(
            r#"{SERVER_HEADER}
[defaults]
network = "container:vpn"

[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
"#
        ));
        assert_eq!(
            compute_config_hash(&a.config.jobs[0]),
            compute_config_hash(&b.config.jobs[0]),
            "network: hash must be stable across job-vs-defaults"
        );
    }

    // volumes
    {
        let a = parse(&format!(
            r#"{SERVER_HEADER}
[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
volumes = ["/host:/c"]
"#
        ));
        let b = parse(&format!(
            r#"{SERVER_HEADER}
[defaults]
volumes = ["/host:/c"]

[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
"#
        ));
        assert_eq!(
            compute_config_hash(&a.config.jobs[0]),
            compute_config_hash(&b.config.jobs[0]),
            "volumes: hash must be stable across job-vs-defaults"
        );
    }

    // timeout
    {
        let a = parse(&format!(
            r#"{SERVER_HEADER}
[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
timeout = "5m"
"#
        ));
        let b = parse(&format!(
            r#"{SERVER_HEADER}
[defaults]
timeout = "5m"

[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
"#
        ));
        assert_eq!(
            compute_config_hash(&a.config.jobs[0]),
            compute_config_hash(&b.config.jobs[0]),
            "timeout: hash must be stable across job-vs-defaults"
        );
    }

    // delete
    {
        let a = parse(&format!(
            r#"{SERVER_HEADER}
[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
delete = true
"#
        ));
        let b = parse(&format!(
            r#"{SERVER_HEADER}
[defaults]
delete = true

[[jobs]]
name = "j"
schedule = "*/5 * * * *"
image = "alpine:latest"
"#
        ));
        assert_eq!(
            compute_config_hash(&a.config.jobs[0]),
            compute_config_hash(&b.config.jobs[0]),
            "delete: hash must be stable across job-vs-defaults"
        );
    }
}

#[test]
fn cmd_preserved_on_docker_job() {
    // The marquee cmd-end-to-end test: TOML cmd = [...] must reach
    // JobConfig.cmd so DockerJobConfig in the executor can pick it up.
    let toml = format!(
        r#"{SERVER_HEADER}
[[jobs]]
name = "cmd-job"
schedule = "*/5 * * * *"
image = "alpine:latest"
cmd = ["echo", "hi"]
"#
    );
    let f = write_toml(&toml);
    let parsed = parse_and_validate(f.path()).expect("must parse");
    let job = &parsed.config.jobs[0];
    assert_eq!(
        job.cmd,
        Some(vec!["echo".to_string(), "hi".to_string()]),
        "TOML cmd must flow through to JobConfig.cmd"
    );
}

#[test]
fn cmd_in_defaults_is_not_merged() {
    // `cmd` is not a defaults-eligible field per spec. Whichever way the
    // TOML parser handles a spurious `cmd = [...]` in [defaults]
    // (silent ignore vs. parser-level reject), the end state must be
    // that a job which omits `cmd` ends up with cmd == None.
    let toml = format!(
        r#"{SERVER_HEADER}
[defaults]
image = "alpine:latest"
cmd = ["echo", "defaulted"]

[[jobs]]
name = "no-cmd-job"
schedule = "*/5 * * * *"
"#
    );
    let f = write_toml(&toml);
    match parse_and_validate(f.path()) {
        Ok(parsed) => {
            // Case (a): TOML parser silently ignored the unknown key.
            // Observed at implementation time: serde without
            // `deny_unknown_fields` accepts the extra key on DefaultsConfig.
            let job = &parsed.config.jobs[0];
            assert_eq!(
                job.cmd, None,
                "spurious [defaults].cmd must NOT leak into a job that omits cmd"
            );
        }
        Err(errors) => {
            // Case (b): TOML parser rejected the unknown key. Either
            // outcome is acceptable; the invariant is that cmd never
            // ends up on the job by accident.
            let mentions_cmd = errors.iter().any(|e: &ConfigError| {
                e.message.contains("cmd") && e.message.contains("[defaults]")
            });
            assert!(
                mentions_cmd,
                "if the parser rejects the unknown key, the error must mention cmd and [defaults]"
            );
        }
    }
}

#[test]
fn cmd_on_command_job_is_rejected() {
    // Pre-fix this was silently accepted and the `cmd` field was dropped
    // from `serialize_config_json` because command jobs never read
    // `config_json` back through DockerJobConfig. Post-fix the validator
    // flags the nonsense config loudly so the operator corrects their
    // intent instead of shipping a silent-no-op.
    let toml = format!(
        r#"{SERVER_HEADER}
[[jobs]]
name = "cmd-on-command"
schedule = "*/5 * * * *"
command = "echo hi"
cmd = ["ignored", "args"]
"#
    );
    let f = write_toml(&toml);
    let errors = parse_and_validate(f.path()).expect_err("validator must reject");
    let cmd_error = errors
        .iter()
        .find(|e: &&ConfigError| e.message.contains("cmd") && e.message.contains("docker jobs"))
        .expect("must have a cmd-on-non-docker error");
    assert!(cmd_error.message.contains("cmd-on-command"));
}

#[test]
fn cmd_on_script_job_is_rejected() {
    let toml = format!(
        r##"{SERVER_HEADER}
[[jobs]]
name = "cmd-on-script"
schedule = "*/5 * * * *"
script = """#!/bin/sh
echo hi
"""
cmd = ["ignored"]
"##
    );
    let f = write_toml(&toml);
    let errors = parse_and_validate(f.path()).expect_err("validator must reject");
    assert!(
        errors
            .iter()
            .any(|e: &ConfigError| e.message.contains("cmd-on-script")
                && e.message.contains("docker jobs"))
    );
}
