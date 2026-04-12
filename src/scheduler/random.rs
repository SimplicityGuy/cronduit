//! @random cron field resolver.
//!
//! Transforms schedule strings containing `@random` tokens into concrete cron values.
//! Enforces minimum spacing between @random jobs via a slot-based algorithm.
//! Handles infeasibility by relaxing the gap with a warning.

use rand::Rng;
use std::time::Duration;

/// Valid ranges for each of the 5 standard cron fields.
const FIELD_RANGES: [(u32, u32); 5] = [
    (0, 59),  // minute
    (0, 23),  // hour
    (1, 31),  // day of month
    (1, 12),  // month
    (0, 6),   // day of week
];

/// Returns true if any whitespace-delimited field in the schedule equals `@random`.
pub fn is_random_schedule(schedule: &str) -> bool {
    todo!()
}

/// Resolve `@random` tokens in a cron schedule to concrete values.
///
/// - If `existing_resolved` is `Some` and the raw schedule hasn't changed
///   (caller determines this via config_hash), returns the existing value.
/// - For each `@random` field, picks a random value from the valid range.
/// - Non-`@random` fields pass through unchanged.
/// - Validates the result with `croner::Cron::from_str()` and retries if invalid.
pub fn resolve_schedule(
    raw: &str,
    existing_resolved: Option<&str>,
    rng: &mut impl Rng,
) -> String {
    todo!()
}

/// Batch-resolve @random schedules with minimum gap enforcement.
///
/// Input: `(job_name, raw_schedule, existing_resolved)`
/// Output: `(job_name, resolved_schedule)`
///
/// Implements slot-based gap enforcement:
/// - Non-random jobs get identity resolution
/// - Random jobs sorted by constraint severity
/// - Feasibility pre-check with gap relaxation for overflow
/// - Retry with slot conflict detection
pub fn resolve_random_schedules_batch(
    jobs: &[(String, String, Option<String>)],
    min_gap: Duration,
    rng: &mut impl Rng,
) -> Vec<(String, String)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn is_random_single_field() {
        assert!(is_random_schedule("@random 14 * * *"));
    }

    #[test]
    fn is_random_not_random() {
        assert!(!is_random_schedule("0 14 * * *"));
    }

    #[test]
    fn is_random_multiple_fields() {
        assert!(is_random_schedule("@random @random * * *"));
    }

    #[test]
    fn resolve_single_random_minute() {
        let mut rng = seeded_rng();
        let result = resolve_schedule("@random 14 * * *", None, &mut rng);
        let fields: Vec<&str> = result.split_whitespace().collect();
        assert_eq!(fields.len(), 5);
        let minute: u32 = fields[0].parse().expect("minute should be a number");
        assert!(minute <= 59, "minute {} out of range", minute);
        assert_eq!(fields[1], "14");
    }

    #[test]
    fn resolve_multiple_random_fields() {
        let mut rng = seeded_rng();
        let result = resolve_schedule("@random @random * * *", None, &mut rng);
        let fields: Vec<&str> = result.split_whitespace().collect();
        assert_eq!(fields.len(), 5);
        let minute: u32 = fields[0].parse().expect("minute should be a number");
        let hour: u32 = fields[1].parse().expect("hour should be a number");
        assert!(minute <= 59);
        assert!(hour <= 23);
    }

    #[test]
    fn resolve_non_random_passthrough() {
        let mut rng = seeded_rng();
        let result = resolve_schedule("0 14 * * *", None, &mut rng);
        assert_eq!(result, "0 14 * * *");
    }

    #[test]
    fn preserve_existing_resolved() {
        let mut rng = seeded_rng();
        // When existing_resolved matches the pattern and raw hasn't changed,
        // should return existing value.
        let result = resolve_schedule(
            "@random 14 * * *",
            Some("42 14 * * *"),
            &mut rng,
        );
        assert_eq!(result, "42 14 * * *");
    }

    #[test]
    fn stable_across_reload() {
        // Existing resolved with same raw schedule => preserved
        let mut rng = seeded_rng();
        let result = resolve_schedule(
            "@random @random * * *",
            Some("15 8 * * *"),
            &mut rng,
        );
        assert_eq!(result, "15 8 * * *", "should preserve existing resolved schedule");
    }

    #[test]
    fn new_resolution_when_raw_changed() {
        let mut rng = seeded_rng();
        // Hour changed from 14 to 15, so existing_resolved should NOT be preserved
        // because the caller should pass None when config_hash differs.
        // However, our function only gets existing_resolved when hash matches.
        // When the schedule changes, caller passes None.
        let result = resolve_schedule("@random 15 * * *", None, &mut rng);
        let fields: Vec<&str> = result.split_whitespace().collect();
        assert_eq!(fields[1], "15");
        // It's a new resolution, so minute is random but valid
        let minute: u32 = fields[0].parse().unwrap();
        assert!(minute <= 59);
    }

    #[test]
    fn resolved_validates_with_croner() {
        let mut rng = seeded_rng();
        let result = resolve_schedule("@random @random @random @random @random", None, &mut rng);
        // Must parse with croner
        let cron = result.parse::<croner::Cron>();
        assert!(cron.is_ok(), "resolved '{}' should be valid cron", result);
    }

    #[test]
    fn batch_gap_enforcement() {
        let mut rng = seeded_rng();
        let jobs: Vec<(String, String, Option<String>)> = (0..3)
            .map(|i| (format!("job-{i}"), "@random @random * * *".to_string(), None))
            .collect();
        let min_gap = Duration::from_secs(5400); // 90 minutes

        let results = resolve_random_schedules_batch(&jobs, min_gap, &mut rng);
        assert_eq!(results.len(), 3);

        // Extract minute-of-day for each
        let minutes: Vec<u32> = results
            .iter()
            .map(|(_, s)| {
                let fields: Vec<&str> = s.split_whitespace().collect();
                let m: u32 = fields[0].parse().unwrap();
                let h: u32 = fields[1].parse().unwrap();
                h * 60 + m
            })
            .collect();

        // Check all pairs differ by at least 90 minutes (wrapping around 24h)
        for i in 0..minutes.len() {
            for j in (i + 1)..minutes.len() {
                let diff = circular_distance(minutes[i], minutes[j], 1440);
                assert!(
                    diff >= 90,
                    "jobs {} and {} are only {} minutes apart (need >= 90)",
                    i, j, diff
                );
            }
        }
    }

    #[test]
    fn infeasible_gap_relaxes() {
        let mut rng = seeded_rng();
        // 30 jobs * 90min = 2700min > 1440min/day => infeasible
        let jobs: Vec<(String, String, Option<String>)> = (0..30)
            .map(|i| (format!("job-{i}"), "@random @random * * *".to_string(), None))
            .collect();
        let min_gap = Duration::from_secs(5400); // 90 minutes

        // Should not panic -- relaxes gap instead
        let results = resolve_random_schedules_batch(&jobs, min_gap, &mut rng);
        assert_eq!(results.len(), 30);

        // All should be valid cron
        for (name, schedule) in &results {
            let cron = schedule.parse::<croner::Cron>();
            assert!(cron.is_ok(), "job {} schedule '{}' should be valid", name, schedule);
        }
    }

    #[test]
    fn batch_non_random_passthrough() {
        let mut rng = seeded_rng();
        let jobs = vec![
            ("fixed".to_string(), "0 14 * * *".to_string(), None),
            ("random".to_string(), "@random @random * * *".to_string(), None),
        ];
        let results = resolve_random_schedules_batch(&jobs, Duration::from_secs(0), &mut rng);
        let fixed = results.iter().find(|(n, _)| n == "fixed").unwrap();
        assert_eq!(fixed.1, "0 14 * * *");
    }

    #[test]
    fn validate_field_count_rejection() {
        let mut rng = seeded_rng();
        // T-05-01: malformed input with wrong field count
        let result = resolve_schedule("@random 14 *", None, &mut rng);
        // Should return the input unchanged (can't resolve malformed)
        assert_eq!(result, "@random 14 *");
    }

    /// Helper: circular distance on a ring of `modulus` size
    fn circular_distance(a: u32, b: u32, modulus: u32) -> u32 {
        let diff = if a > b { a - b } else { b - a };
        diff.min(modulus - diff)
    }
}
