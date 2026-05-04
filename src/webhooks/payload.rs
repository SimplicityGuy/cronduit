//! JSON wire-format payload for webhook deliveries (Phase 18 / WH-09).
//!
//! Distinct from `src/webhooks/event.rs` (channel-message contract). This
//! module owns the 16-field v1 schema serialized to compact JSON and HMAC-
//! signed by `HttpDispatcher`. `payload_version: "v1"` is locked for the
//! entire v1.2 line — future additions are additive (new optional fields
//! only); breaking changes require `payload_version: "v2"` (a future phase).
//!
//! NOTE: Field order in the struct == serialization order. `serde_derive`
//! emits fields in declaration order; downstream HMAC compares depend on
//! deterministic byte output. Pitfall B in 18-RESEARCH.md.

use chrono::SecondsFormat;
use serde::Serialize;

use crate::db::queries::{DbRunDetail, FailureContext};
use crate::webhooks::event::RunFinalized;

/// Locked 16-field Standard Webhooks v1 payload. Per WH-09 / D-06.
/// Field declaration order MUST match D-06 listed order — `serde_derive`
/// emits fields in declaration order, and HMAC signing depends on
/// deterministic byte output.
#[derive(Debug, Serialize)]
pub struct WebhookPayload<'a> {
    /// "v1" — D-08 lock for v1.2 line.
    pub payload_version: &'static str,
    /// "run_finalized" — D-06.
    pub event_type: &'static str,
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: &'a str,
    pub status: &'a str,
    pub exit_code: Option<i32>,
    /// RFC3339 with `Z` suffix (Pitfall F: NOT `+00:00`; produced by
    /// `to_rfc3339_opts(SecondsFormat::Secs, true)`).
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
    /// Filter-matching stream position (Phase 18 / WH-06 / D-12, D-15).
    /// Differs from `consecutive_failures` — see field below.
    pub streak_position: i64,
    /// Phase 16's unified streak count (failed | timeout | error since
    /// last success). Diverges from `streak_position` on purpose — D-07.
    pub consecutive_failures: i64,
    /// `null` for non-docker jobs (D-07). Always emitted (not omitted)
    /// for schema stability; receivers can index without `KeyError`.
    pub image_digest: Option<String>,
    /// `null` for pre-v1.2 rows; populated for runs after Phase 16's
    /// `job_runs.config_hash` column landed.
    pub config_hash: Option<String>,
    /// Real values from `jobs.tags` column via `DbRunDetail.tags` (Phase 22
    /// WH-09 / D-05). Sorted-canonical order. Always emitted (never omitted)
    /// for schema stability; receivers can index without `KeyError`. Per
    /// WH-09 the field is part of the locked v1.2.0 payload schema —
    /// future additions are additive only.
    pub tags: Vec<String>,
    /// `env!("CARGO_PKG_VERSION")` baked at compile time (D-07; aligns
    /// with `feedback_tag_release_version_match.md` project memory).
    pub cronduit_version: &'static str,
}

impl<'a> WebhookPayload<'a> {
    /// Construct a payload from the channel event + the DB-side context.
    /// `filter_position` is the WH-06 streak-position computed by
    /// `crate::webhooks::coalesce::filter_position`.
    /// `cronduit_version` is passed in (rather than calling `env!` here)
    /// so the dispatcher can pre-compute it once and avoid repeated lookups.
    pub fn build(
        event: &'a RunFinalized,
        fctx: &FailureContext,
        run: &DbRunDetail,
        filter_position: i64,
        cronduit_version: &'static str,
    ) -> Self {
        let duration_ms = (event.finished_at - event.started_at).num_milliseconds();
        Self {
            payload_version: "v1",
            event_type: "run_finalized",
            run_id: event.run_id,
            job_id: event.job_id,
            job_name: &event.job_name,
            status: &event.status,
            exit_code: event.exit_code,
            started_at: event.started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            finished_at: event.finished_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            duration_ms,
            streak_position: filter_position,
            consecutive_failures: fctx.consecutive_failures,
            image_digest: run.image_digest.clone(),
            config_hash: run.config_hash.clone(),
            tags: run.tags.clone(), // Phase 22 WH-09 / D-05
            cronduit_version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn fixture_event() -> RunFinalized {
        RunFinalized {
            run_id: 42,
            job_id: 7,
            job_name: "backup-nightly".to_string(),
            status: "failed".to_string(),
            exit_code: Some(1),
            started_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 43, 11).unwrap(),
            finished_at: Utc.with_ymd_and_hms(2026, 4, 29, 10, 43, 12).unwrap(),
        }
    }

    fn fixture_fctx() -> FailureContext {
        FailureContext {
            consecutive_failures: 3,
            last_success_run_id: Some(40),
            last_success_image_digest: Some("sha256:abc".to_string()),
            last_success_config_hash: Some("hash-x".to_string()),
        }
    }

    fn fixture_run_detail(
        image_digest: Option<String>,
        config_hash: Option<String>,
    ) -> DbRunDetail {
        DbRunDetail {
            id: 42,
            job_id: 7,
            job_run_number: 12,
            job_name: "backup-nightly".to_string(),
            status: "failed".to_string(),
            trigger: "scheduled".to_string(),
            start_time: "2026-04-29T10:43:11Z".to_string(),
            end_time: Some("2026-04-29T10:43:12Z".to_string()),
            duration_ms: Some(1000),
            exit_code: Some(1),
            error_message: None,
            image_digest,
            config_hash,
            scheduled_for: None, // Phase 21 FCTX-06: test fixture
            tags: Vec::new(),    // Phase 22: defaulted; fixture_run_detail_with_tags overrides
        }
    }

    /// Phase 22 WH-09 / D-05: 3-arg variant for tests that need to seed
    /// non-empty tag values into `DbRunDetail.tags`. Backwards-compatible
    /// with the seven existing `fixture_run_detail(None, None)` callers.
    fn fixture_run_detail_with_tags(
        image_digest: Option<String>,
        config_hash: Option<String>,
        tags: Vec<String>,
    ) -> DbRunDetail {
        let mut r = fixture_run_detail(image_digest, config_hash);
        r.tags = tags;
        r
    }

    #[test]
    fn payload_payload_version_is_v1() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(Some("sha256:cur".into()), Some("h-cur".into()));
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        assert_eq!(p.payload_version, "v1");
    }

    #[test]
    fn payload_event_type_is_run_finalized() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(None, None);
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        assert_eq!(p.event_type, "run_finalized");
    }

    #[test]
    fn payload_contains_all_16_fields() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(Some("sha256:cur".into()), Some("h-cur".into()));
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        let bytes = serde_json::to_vec(&p).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        for k in [
            "\"payload_version\":",
            "\"event_type\":",
            "\"run_id\":",
            "\"job_id\":",
            "\"job_name\":",
            "\"status\":",
            "\"exit_code\":",
            "\"started_at\":",
            "\"finished_at\":",
            "\"duration_ms\":",
            "\"streak_position\":",
            "\"consecutive_failures\":",
            "\"image_digest\":",
            "\"config_hash\":",
            "\"tags\":",
            "\"cronduit_version\":",
        ] {
            assert!(s.contains(k), "field {k} missing from payload JSON: {s}");
        }
    }

    #[test]
    fn payload_serializes_deterministically_to_compact_json() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(Some("sha256:cur".into()), Some("h-cur".into()));
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        let bytes_a = serde_json::to_vec(&p).unwrap();
        let bytes_b = serde_json::to_vec(&p).unwrap();
        assert_eq!(
            bytes_a, bytes_b,
            "two serializations must produce identical bytes (Pitfall B)"
        );
        assert!(
            !bytes_a.contains(&b'\n'),
            "compact JSON must have no newlines (Pitfall C)"
        );
    }

    #[test]
    fn payload_field_order_matches_struct_declaration() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(None, None);
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        let bytes = serde_json::to_vec(&p).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        let pos_v = s.find("payload_version").unwrap();
        let pos_e = s.find("event_type").unwrap();
        let pos_r = s.find("run_id").unwrap();
        assert!(pos_v < pos_e, "payload_version must precede event_type");
        assert!(pos_e < pos_r, "event_type must precede run_id");
    }

    #[test]
    fn payload_image_digest_null_when_none() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(None, None);
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"image_digest\":null"));
        assert!(s.contains("\"config_hash\":null"));
    }

    #[test]
    fn payload_tags_carries_real_values() {
        // Phase 22 WH-09 / D-05 / D-06.5: the placeholder is gone.
        // Receivers see real tag values from the jobs.tags column,
        // round-tripped through DbRunDetail.tags into the wire JSON.
        // Sorted-canonical order is emitted (operator-written
        // ["weekly", "backup"] becomes ["backup", "weekly"] after the
        // upsert path's normalize+sort+dedup; this test asserts the
        // ORDER in the wire payload).
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail_with_tags(
            None,
            None,
            vec!["backup".to_string(), "weekly".to_string()],
        );
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        let s = serde_json::to_string(&p).unwrap();
        assert!(
            s.contains(r#""tags":["backup","weekly"]"#),
            "tags must round-trip into payload preserving sorted-canonical order: {s}"
        );
    }

    #[test]
    fn payload_timestamps_use_z_suffix() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(None, None);
        let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
        assert!(
            p.started_at.ends_with('Z'),
            "started_at must end with Z (Pitfall F): {}",
            p.started_at
        );
        assert!(
            p.finished_at.ends_with('Z'),
            "finished_at must end with Z (Pitfall F): {}",
            p.finished_at
        );
        assert!(
            !p.started_at.contains("+00:00"),
            "must not use +00:00 (Pitfall F)"
        );
    }

    #[test]
    fn payload_cronduit_version_from_env_macro() {
        let event = fixture_event();
        let fctx = fixture_fctx();
        let run = fixture_run_detail(None, None);
        // The dispatcher will pass env!("CARGO_PKG_VERSION") at construction;
        // verify build() honors what is passed in.
        let p = WebhookPayload::build(&event, &fctx, &run, 1, env!("CARGO_PKG_VERSION"));
        assert_eq!(p.cronduit_version, env!("CARGO_PKG_VERSION"));
    }
}
