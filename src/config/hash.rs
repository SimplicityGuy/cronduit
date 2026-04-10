use super::JobConfig;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

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
    // DO NOT include `env` -- its values are SecretString and must not be hashed/logged.

    let bytes = serde_json::to_vec(&map).expect("serde_json BTreeMap never fails");
    let mut h = Sha256::new();
    h.update(&bytes);
    format!("{:x}", h.finalize())
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
            network: None,
            container_name: None,
            timeout: None,
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
}
