use super::JobConfig;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Compute a stable SHA-256 hex digest of a job's config (D-15).
///
/// The normalization is: serialize the non-secret subset of JobConfig
/// into a BTreeMap<String, serde_json::Value> (sorted keys) and then
/// `serde_json::to_vec` it -- serde_json preserves BTreeMap key order.
///
/// NOTE: Phase 1 does NOT write data to the `config_hash` column yet;
/// the column exists so Phase 2's sync engine does not require another
/// migration. This function is therefore unit-tested but not called
/// from the run path in Phase 1.
pub fn compute_config_hash(job: &JobConfig) -> String {
    let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    map.insert("name", serde_json::json!(job.name));
    map.insert("schedule", serde_json::json!(job.schedule));
    if let Some(c) = &job.command {
        map.insert("command", serde_json::json!(c));
    }
    if let Some(s) = &job.script {
        map.insert("script", serde_json::json!(s));
    }
    if let Some(i) = &job.image {
        map.insert("image", serde_json::json!(i));
    }
    if let Some(v) = &job.volumes {
        map.insert("volumes", serde_json::json!(v));
    }
    if let Some(n) = &job.network {
        map.insert("network", serde_json::json!(n));
    }
    if let Some(cn) = &job.container_name {
        map.insert("container_name", serde_json::json!(cn));
    }
    if let Some(t) = &job.timeout {
        map.insert("timeout_secs", serde_json::json!(t.as_secs()));
    }
    if let Some(d) = &job.delete {
        map.insert("delete", serde_json::json!(d));
    }
    if let Some(c) = &job.cmd {
        map.insert("cmd", serde_json::json!(c));
    }
    if let Some(l) = &job.labels {
        map.insert("labels", serde_json::json!(l));
    }
    // DO NOT include `env` -- its values are SecretString and must not be hashed/logged.

    let bytes = serde_json::to_vec(&map).expect("serde_json BTreeMap never fails");
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest = h.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn mk_job() -> JobConfig {
        JobConfig {
            name: "t".into(),
            schedule: "*/5 * * * *".into(),
            command: Some("echo hi".into()),
            script: None,
            image: None,
            use_defaults: None,
            env: BTreeMap::new(),
            volumes: None,
            labels: None,
            network: None,
            container_name: None,
            timeout: None,
            delete: None,
            cmd: None,
        }
    }

    #[test]
    fn hash_is_stable() {
        let a = compute_config_hash(&mk_job());
        let b = compute_config_hash(&mk_job());
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn hash_differs_on_name_change() {
        let mut j = mk_job();
        let a = compute_config_hash(&j);
        j.name = "t2".into();
        let b = compute_config_hash(&j);
        assert_ne!(a, b);
    }

    #[test]
    fn hash_stable_across_defaults_merge() {
        // Guards Warning 3: a future refactor of parse_and_validate that
        // drops fields during the apply_defaults rebuild must not silently
        // change hashes. For each defaults-eligible field, the field set
        // directly on the job must hash identically to the field set in
        // [defaults] then merged in via apply_defaults.
        //
        // Docker-only fields (image/network/volumes/delete) are gated on
        // `is_non_docker == false`, so we build mk_docker_job() which has
        // command=None and image already set (or will be merged in).
        use crate::config::DefaultsConfig;
        use crate::config::defaults::apply_defaults;

        // Helper: a docker job stub (command=None) so docker-only field
        // merging is reachable. mk_job() is a command job by default and
        // would skip every docker-only branch.
        fn mk_docker_job() -> JobConfig {
            JobConfig {
                name: "t".into(),
                schedule: "*/5 * * * *".into(),
                command: None,
                script: None,
                image: Some("alpine:latest".into()),
                use_defaults: None,
                env: BTreeMap::new(),
                volumes: None,
                labels: None,
                network: None,
                container_name: None,
                timeout: None,
                delete: None,
                cmd: None,
            }
        }

        // image (docker job; need a non-docker baseline for the no-image
        // representation, so use an empty job with no command/script/image
        // and let apply_defaults fill image from defaults).
        {
            let a = mk_docker_job();
            // b has no image field set; defaults provides "alpine:latest"
            let b = JobConfig {
                image: None,
                ..mk_docker_job()
            };
            let defaults = DefaultsConfig {
                image: Some("alpine:latest".into()),
                network: None,
                volumes: None,
                labels: None,
                delete: None,
                timeout: None,
                random_min_gap: None,
            };
            let b = apply_defaults(b, Some(&defaults));
            assert_eq!(
                compute_config_hash(&a),
                compute_config_hash(&b),
                "image: field-on-job vs field-from-defaults"
            );
        }

        // network (docker job)
        {
            let mut a = mk_docker_job();
            a.network = Some("container:vpn".into());
            let b = mk_docker_job();
            let defaults = DefaultsConfig {
                image: None,
                network: Some("container:vpn".into()),
                volumes: None,
                labels: None,
                delete: None,
                timeout: None,
                random_min_gap: None,
            };
            let b = apply_defaults(b, Some(&defaults));
            assert_eq!(
                compute_config_hash(&a),
                compute_config_hash(&b),
                "network: field-on-job vs field-from-defaults"
            );
        }

        // volumes (docker job)
        {
            let mut a = mk_docker_job();
            a.volumes = Some(vec!["/host:/container".to_string()]);
            let b = mk_docker_job();
            let defaults = DefaultsConfig {
                image: None,
                network: None,
                volumes: Some(vec!["/host:/container".to_string()]),
                labels: None,
                delete: None,
                timeout: None,
                random_min_gap: None,
            };
            let b = apply_defaults(b, Some(&defaults));
            assert_eq!(
                compute_config_hash(&a),
                compute_config_hash(&b),
                "volumes: field-on-job vs field-from-defaults"
            );
        }

        // timeout (works for any job type -- mk_job is fine here)
        {
            let mut a = mk_job();
            a.timeout = Some(std::time::Duration::from_secs(300));
            let b = mk_job();
            let defaults = DefaultsConfig {
                image: None,
                network: None,
                volumes: None,
                labels: None,
                delete: None,
                timeout: Some(std::time::Duration::from_secs(300)),
                random_min_gap: None,
            };
            let b = apply_defaults(b, Some(&defaults));
            assert_eq!(
                compute_config_hash(&a),
                compute_config_hash(&b),
                "timeout: field-on-job vs field-from-defaults"
            );
        }

        // delete (docker job; the new field) -- REQUIRED to guard Warning 3.
        {
            let mut a = mk_docker_job();
            a.delete = Some(true);
            let b = mk_docker_job();
            let defaults = DefaultsConfig {
                image: None,
                network: None,
                volumes: None,
                labels: None,
                delete: Some(true),
                timeout: None,
                random_min_gap: None,
            };
            let b = apply_defaults(b, Some(&defaults));
            assert_eq!(
                compute_config_hash(&a),
                compute_config_hash(&b),
                "delete: field-on-job vs field-from-defaults"
            );
        }
    }

    #[test]
    fn hash_differs_on_delete_change() {
        // Guards change-detection: an operator toggling [defaults].delete
        // must produce a different config_hash so sync_config_to_db
        // classifies the job as `updated`, not `unchanged`.
        let mut a = mk_job();
        a.delete = Some(true);
        let mut b = mk_job();
        b.delete = Some(false);
        assert_ne!(compute_config_hash(&a), compute_config_hash(&b));
    }

    #[test]
    fn hash_differs_on_cmd_change() {
        // Guards change-detection for the new per-job `cmd` field. Four
        // cases must be pairwise distinct, including the subtle
        // None vs Some(vec![]) distinction (image's baked-in CMD vs
        // explicit "no command" override).
        let mut a = mk_job();
        a.cmd = Some(vec!["a".to_string()]);
        let mut b = mk_job();
        b.cmd = Some(vec!["b".to_string()]);
        let mut c = mk_job();
        c.cmd = None;
        let mut d = mk_job();
        d.cmd = Some(vec![]);

        let ha = compute_config_hash(&a);
        let hb = compute_config_hash(&b);
        let hc = compute_config_hash(&c);
        let hd = compute_config_hash(&d);

        assert_ne!(ha, hb, "Some([\"a\"]) vs Some([\"b\"])");
        assert_ne!(ha, hc, "Some([\"a\"]) vs None");
        assert_ne!(ha, hd, "Some([\"a\"]) vs Some([])");
        assert_ne!(hb, hc, "Some([\"b\"]) vs None");
        assert_ne!(hb, hd, "Some([\"b\"]) vs Some([])");
        assert_ne!(
            hc, hd,
            "None vs Some([]) -- image CMD vs explicit empty override"
        );
    }

    #[test]
    fn hash_differs_on_labels_change() {
        // Guards change-detection for the new per-job `labels` field
        // (LBL-01 / Layer 3). An operator editing a label value must
        // produce a different config_hash so sync_config_to_db classifies
        // the job as `updated`, not `unchanged`.
        //
        // Mirror of `hash_differs_on_cmd_change` shape: two jobs that
        // differ ONLY in labels must hash to different values.
        let mut a = mk_job();
        let mut la = std::collections::HashMap::new();
        la.insert("watchtower.enable".to_string(), "false".to_string());
        a.labels = Some(la);

        let mut b = mk_job();
        let mut lb = std::collections::HashMap::new();
        lb.insert("watchtower.enable".to_string(), "true".to_string()); // value differs
        b.labels = Some(lb);

        assert_ne!(
            compute_config_hash(&a),
            compute_config_hash(&b),
            "hash must change when label value changes (per LBL-01)"
        );
    }
}
