//! Pure-Rust exit-code bucket categorizer + aggregator (Phase 21, EXIT-01..EXIT-05).
//!
//! Status-discriminator-wins classifier (CONTEXT D-08):
//!   status="success"          -> None        (handled by success-rate stat per EXIT-03)
//!   status="stopped"          -> Bucket::Stopped  (regardless of exit_code 137 — cronduit's SIGKILL path)
//!   exit_code IS NULL         -> Bucket::Null  (timeout / cancelled-without-code)
//!   exit_code == Some(1)      -> Bucket::Bucket1
//!   exit_code == Some(2)      -> Bucket::Bucket2
//!   exit_code in 3..=9        -> Bucket::Bucket3to9
//!   exit_code in 10..=126     -> Bucket::Bucket10to126
//!   exit_code == Some(127)    -> Bucket::Bucket127
//!   exit_code in 128..=143    -> Bucket::Bucket128to143  (status='failed'+exit=137 lands here, NOT BucketStopped — EXIT-04)
//!   exit_code in 144..=254    -> Bucket::Bucket144to254
//!   exit_code == Some(255)    -> Bucket::Bucket255
//!
//! Aggregation (`aggregate`) returns a HistogramCard with bucket counts, top-3 codes
//! by frequency with last-seen timestamps (EXIT-05), success-rate (D-09:
//! success_count / (sample_count - stopped_count); None when denominator==0),
//! sample_count, and has_min_samples (D-11: N >= 5).
//!
//! D-15 / D-16: When `sample_count < 5` (or 0 for a brand-new job), `has_min_samples`
//! is false and the consumer (job_detail handler view-model in plan 21-05) renders
//! a "Not enough data yet" empty state instead of a misleading histogram.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitBucket {
    Bucket1,
    Bucket2,
    Bucket3to9,
    Bucket10to126,
    Bucket127,
    Bucket128to143,
    Bucket144to254,
    Bucket255,
    BucketNull,
    BucketStopped,
}

/// Single top-N entry for the EXIT-05 "Most frequent codes" sub-table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopCode {
    pub code: i32,                 // raw exit_code (e.g., 137, 127, 143); BucketNull excluded
    pub count: usize,
    pub last_seen: Option<String>, // RFC3339 end_time of most recent occurrence
}

/// Aggregated card payload consumed by the job_detail handler view-model builder.
#[derive(Debug, Clone)]
pub struct HistogramCard {
    pub buckets: HashMap<ExitBucket, usize>, // 10 keys when present; 0-count buckets may be omitted
    pub success_count: usize,                // status='success' count (NOT a bucket; EXIT-03 stat badge)
    pub stopped_count: usize,                // status='stopped' count (excluded from success-rate denom; D-09)
    pub sample_count: usize,                 // total rows fed in
    pub has_min_samples: bool,               // sample_count >= 5 (D-11)
    pub success_rate: Option<f64>,           // success_count / (sample_count - stopped_count); None when denom == 0
    pub top_codes: Vec<TopCode>,             // top-3 by count, length <= 3 (EXIT-05)
}

/// Returns the bucket for a (status, exit_code) pair, or None for the success path.
///
/// Status-discriminator-wins (D-08):
/// - `status == "success"` always returns `None` regardless of exit_code (success
///   does not occupy a bucket — it's reported via the success-rate stat per EXIT-03).
/// - `status == "stopped"` always returns `Some(BucketStopped)` regardless of
///   exit_code. This is cronduit's operator-stop path; the SIGKILL-induced exit
///   code 137 is incidental, NOT a classifier signal (EXIT-04).
/// - Otherwise (`failed` / `timeout` / `cancelled` / etc.), bucket by exit_code:
///   `None` (timeout-without-code) → `BucketNull`; the four named single-codes
///   (1, 2, 127, 255); the four ranges (3..=9, 10..=126, 128..=143, 144..=254);
///   defensive fallback for any code outside POSIX 0..=255 → `BucketNull`.
pub fn categorize(status: &str, exit_code: Option<i32>) -> Option<ExitBucket> {
    if status == "success" {
        return None;
    }
    if status == "stopped" {
        return Some(ExitBucket::BucketStopped);
    }
    match exit_code {
        None => Some(ExitBucket::BucketNull),
        Some(1) => Some(ExitBucket::Bucket1),
        Some(2) => Some(ExitBucket::Bucket2),
        Some(127) => Some(ExitBucket::Bucket127),
        Some(255) => Some(ExitBucket::Bucket255),
        Some(c) if (3..=9).contains(&c) => Some(ExitBucket::Bucket3to9),
        Some(c) if (10..=126).contains(&c) => Some(ExitBucket::Bucket10to126),
        Some(c) if (128..=143).contains(&c) => Some(ExitBucket::Bucket128to143),
        Some(c) if (144..=254).contains(&c) => Some(ExitBucket::Bucket144to254),
        // Negative or > 255: route to BucketNull as a defensive fallback (operators
        // won't see real codes outside 0..=255 from POSIX wait-status semantics).
        Some(_) => Some(ExitBucket::BucketNull),
    }
}

/// Aggregates raw last-N rows into a HistogramCard.
///
/// Input rows: `(status, exit_code, end_time_rfc3339)` — owned tuples per
/// research §C, sourced from `queries::get_recent_runs_for_histogram` which
/// returns rows ordered by `start_time DESC` (newest first).
///
/// Single linear walk:
/// - Tally `categorize()` output into the buckets HashMap.
/// - Count `success` and `stopped` separately for D-09 success-rate.
/// - Build a per-code `(count, latest_end_time)` map for the EXIT-05 top-3.
/// - Compute success_rate = success / (sample - stopped); `None` when denom == 0.
pub fn aggregate(rows: &[(String, Option<i32>, Option<String>)]) -> HistogramCard {
    let sample_count = rows.len();
    let has_min_samples = sample_count >= 5;

    let mut buckets: HashMap<ExitBucket, usize> = HashMap::new();
    let mut success_count: usize = 0;
    let mut stopped_count: usize = 0;
    // For top-3: code -> (count, latest_end_time)
    let mut by_code: HashMap<i32, (usize, Option<String>)> = HashMap::new();

    for (status, exit_code, end_time) in rows {
        if status == "success" {
            success_count += 1;
            continue;
        }
        if status == "stopped" {
            stopped_count += 1;
        }
        if let Some(b) = categorize(status.as_str(), *exit_code) {
            *buckets.entry(b).or_insert(0) += 1;
        }
        // Track raw codes for top-3 (skip None — those land in BucketNull, no per-code identity).
        if let Some(code) = exit_code {
            let entry = by_code.entry(*code).or_insert((0, None));
            entry.0 += 1;
            // Keep the latest end_time (rows arrive ORDER BY start_time DESC, so the FIRST
            // occurrence is the latest; keep the first-seen end_time per code).
            if entry.1.is_none() {
                entry.1 = end_time.clone();
            }
        }
    }

    // Top-3 by count DESC, ties broken by code ASC for determinism.
    let mut top: Vec<TopCode> = by_code
        .into_iter()
        .map(|(code, (count, last_seen))| TopCode {
            code,
            count,
            last_seen,
        })
        .collect();
    top.sort_by(|a, b| b.count.cmp(&a.count).then(a.code.cmp(&b.code)));
    top.truncate(3);

    // Success-rate per D-09: success / (sample - stopped). None when denom is 0.
    let denom = sample_count.saturating_sub(stopped_count);
    let success_rate = if denom == 0 {
        None
    } else {
        Some(success_count as f64 / denom as f64)
    };

    HistogramCard {
        buckets,
        success_count,
        stopped_count,
        sample_count,
        has_min_samples,
        success_rate,
        top_codes: top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorize_all_10_buckets() {
        // Cover every bucket variant. Maps each canonical (status, exit_code) input
        // to its expected ExitBucket. Includes corner exit codes 1, 2, 3, 9, 10, 126,
        // 127, 128, 137, 143, 144, 254, 255, AND the success path returning None.
        assert_eq!(categorize("success", Some(0)), None);
        assert_eq!(categorize("failed", Some(1)), Some(ExitBucket::Bucket1));
        assert_eq!(categorize("failed", Some(2)), Some(ExitBucket::Bucket2));
        assert_eq!(categorize("failed", Some(3)), Some(ExitBucket::Bucket3to9));
        assert_eq!(categorize("failed", Some(9)), Some(ExitBucket::Bucket3to9));
        assert_eq!(
            categorize("failed", Some(10)),
            Some(ExitBucket::Bucket10to126)
        );
        assert_eq!(
            categorize("failed", Some(126)),
            Some(ExitBucket::Bucket10to126)
        );
        assert_eq!(categorize("failed", Some(127)), Some(ExitBucket::Bucket127));
        assert_eq!(
            categorize("failed", Some(128)),
            Some(ExitBucket::Bucket128to143)
        );
        assert_eq!(
            categorize("failed", Some(143)),
            Some(ExitBucket::Bucket128to143)
        );
        assert_eq!(
            categorize("failed", Some(144)),
            Some(ExitBucket::Bucket144to254)
        );
        assert_eq!(
            categorize("failed", Some(254)),
            Some(ExitBucket::Bucket144to254)
        );
        assert_eq!(categorize("failed", Some(255)), Some(ExitBucket::Bucket255));
        assert_eq!(categorize("timeout", None), Some(ExitBucket::BucketNull));
        // Defensive fallback: codes outside POSIX 0..=255 land in BucketNull.
        assert_eq!(categorize("failed", Some(-1)), Some(ExitBucket::BucketNull));
        assert_eq!(
            categorize("failed", Some(999)),
            Some(ExitBucket::BucketNull)
        );
    }

    #[test]
    fn status_discriminator_wins_137() {
        // EXIT-04 dual-classifier: status='stopped'+exit=137 → BucketStopped (NOT 128-143).
        assert_eq!(
            categorize("stopped", Some(137)),
            Some(ExitBucket::BucketStopped)
        );
        // status='failed'+exit=137 → Bucket128to143 (external SIGTERM/SIGSEGV; NOT operator-stop).
        assert_eq!(
            categorize("failed", Some(137)),
            Some(ExitBucket::Bucket128to143)
        );
        // status='stopped' wins regardless of exit_code value.
        assert_eq!(
            categorize("stopped", Some(0)),
            Some(ExitBucket::BucketStopped)
        );
        assert_eq!(
            categorize("stopped", None),
            Some(ExitBucket::BucketStopped)
        );
        assert_eq!(
            categorize("stopped", Some(255)),
            Some(ExitBucket::BucketStopped)
        );
    }

    #[test]
    fn success_rate_excludes_stopped() {
        // D-09: success / (sample - stopped). When all-stopped, denom=0 → None.
        let rows = vec![
            ("success".into(), Some(0), Some("t1".into())),
            ("success".into(), Some(0), Some("t2".into())),
            ("failed".into(), Some(1), Some("t3".into())),
            ("stopped".into(), Some(137), Some("t4".into())),
        ];
        let card = aggregate(&rows);
        // 2 success, 1 failed (counts in denom), 1 stopped (excluded).
        // success_rate = 2 / (4 - 1) = 2/3.
        assert!((card.success_rate.unwrap() - 0.6666666666666666_f64).abs() < 1e-9);
        assert_eq!(card.stopped_count, 1);
        assert_eq!(card.success_count, 2);
        assert_eq!(card.sample_count, 4);

        // All-stopped → denom=0 → None.
        let all_stopped = vec![("stopped".into(), Some(137), Some("t".into()))];
        assert_eq!(aggregate(&all_stopped).success_rate, None);

        // No rows → None.
        assert_eq!(aggregate(&[]).success_rate, None);
    }

    #[test]
    fn top_3_codes_last_seen() {
        // EXIT-05: top-3 codes by count DESC, ties by code ASC for determinism.
        // Rows arrive ORDER BY start_time DESC, so the first occurrence per code
        // is the most recent — that's the last_seen we record.
        let rows = vec![
            ("failed".into(), Some(1), Some("t1".into())),
            ("failed".into(), Some(1), Some("t0".into())), // earlier; first occurrence (latest by ORDER BY DESC) is t1
            ("failed".into(), Some(127), Some("t3".into())),
            ("failed".into(), Some(143), Some("t2".into())),
            ("failed".into(), Some(143), Some("t1".into())),
        ];
        let card = aggregate(&rows);
        assert_eq!(card.top_codes.len(), 3);
        // 1 has count 2; 143 has count 2 — tie broken by code ASC → 1 first.
        assert_eq!(card.top_codes[0].code, 1);
        assert_eq!(card.top_codes[0].count, 2);
        assert_eq!(card.top_codes[0].last_seen, Some("t1".into()));
        assert_eq!(card.top_codes[1].code, 143);
        assert_eq!(card.top_codes[1].count, 2);
        assert_eq!(card.top_codes[1].last_seen, Some("t2".into()));
        assert_eq!(card.top_codes[2].code, 127);
        assert_eq!(card.top_codes[2].count, 1);
        assert_eq!(card.top_codes[2].last_seen, Some("t3".into()));
    }

    #[test]
    fn below_min_samples_threshold() {
        // D-11/D-15/D-16: has_min_samples is false when sample_count < 5.
        let four = vec![("failed".into(), Some(1), Some("t".into())); 4];
        assert!(!aggregate(&four).has_min_samples);
        let five = vec![("failed".into(), Some(1), Some("t".into())); 5];
        assert!(aggregate(&five).has_min_samples);
        let six = vec![("failed".into(), Some(1), Some("t".into())); 6];
        assert!(aggregate(&six).has_min_samples);
    }

    #[test]
    fn zero_samples_brand_new_job() {
        // D-16: brand-new job (zero runs) — same as below-N=5 path.
        let card = aggregate(&[]);
        assert_eq!(card.sample_count, 0);
        assert!(!card.has_min_samples);
        assert_eq!(card.success_rate, None);
        assert!(card.buckets.is_empty());
        assert!(card.top_codes.is_empty());
        assert_eq!(card.success_count, 0);
        assert_eq!(card.stopped_count, 0);
    }
}
