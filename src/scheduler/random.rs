//! @random cron field resolver.
//!
//! Transforms schedule strings containing `@random` tokens into concrete cron values.
//! Enforces minimum spacing between @random jobs via a slot-based algorithm.
//! Handles infeasibility by relaxing the gap with a warning.
//!
//! T-05-01: Validates field count before resolution; rejects malformed input.
//! T-05-02: Caps retry attempts and relaxes infeasible gaps to guarantee termination.

use rand::Rng;
use std::str::FromStr;
use std::time::Duration;

/// Valid ranges for each of the 5 standard cron fields.
const FIELD_RANGES: [(u32, u32); 5] = [
    (0, 59),  // minute
    (0, 23),  // hour
    (1, 31),  // day of month
    (1, 12),  // month
    (0, 6),   // day of week
];

/// Minutes in a day, used for circular gap calculations.
const MINUTES_IN_DAY: u32 = 1440;

/// Maximum retry attempts for resolving a single schedule to a valid cron expression.
const MAX_RESOLVE_RETRIES: u32 = 10;

/// Maximum retry attempts per job for slot-based gap enforcement.
const MAX_SLOT_RETRIES: u32 = 100;

/// Returns true if any whitespace-delimited field in the schedule equals `@random`.
pub fn is_random_schedule(schedule: &str) -> bool {
    schedule.split_whitespace().any(|f| f == "@random")
}

/// Resolve `@random` tokens in a cron schedule to concrete values.
///
/// - If `existing_resolved` is `Some`, returns the existing value (caller is
///   responsible for only passing `Some` when config_hash matches).
/// - For each `@random` field, picks a random value from the valid range.
/// - Non-`@random` fields pass through unchanged.
/// - Validates the result with `croner::Cron::from_str()` and retries if invalid.
/// - T-05-01: Rejects schedules that don't have exactly 5 fields.
pub fn resolve_schedule(
    raw: &str,
    existing_resolved: Option<&str>,
    rng: &mut impl Rng,
) -> String {
    // If we have an existing resolved schedule, preserve it (stability across reloads).
    if let Some(existing) = existing_resolved {
        return existing.to_string();
    }

    let fields: Vec<&str> = raw.split_whitespace().collect();

    // T-05-01: Validate exactly 5 fields. Return raw unchanged if malformed.
    if fields.len() != 5 {
        tracing::warn!(
            target: "cronduit.random",
            schedule = %raw,
            field_count = fields.len(),
            "malformed schedule: expected 5 fields, returning unchanged"
        );
        return raw.to_string();
    }

    // If no @random tokens, pass through unchanged.
    if !is_random_schedule(raw) {
        return raw.to_string();
    }

    // Resolve with retry for croner validation.
    for _ in 0..MAX_RESOLVE_RETRIES {
        let resolved = resolve_fields(&fields, rng);
        if validate_cron(&resolved) {
            return resolved;
        }
    }

    // Last-ditch attempt: accept whatever we get.
    tracing::warn!(
        target: "cronduit.random",
        schedule = %raw,
        "failed to resolve valid cron after {} attempts, accepting best effort",
        MAX_RESOLVE_RETRIES
    );
    resolve_fields(&fields, rng)
}

/// Replace @random fields with random values from the appropriate range.
fn resolve_fields(fields: &[&str], rng: &mut impl Rng) -> String {
    fields
        .iter()
        .enumerate()
        .map(|(i, &field)| {
            if field == "@random" {
                let (min, max) = FIELD_RANGES[i];
                rng.gen_range(min..=max).to_string()
            } else {
                field.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Validate that a resolved cron string is parseable by croner.
fn validate_cron(schedule: &str) -> bool {
    croner::Cron::from_str(schedule).is_ok()
}

/// Extract the minute-of-day from a resolved 5-field cron schedule.
/// Returns `hour * 60 + minute` for gap calculations.
fn minute_of_day(schedule: &str) -> Option<u32> {
    let fields: Vec<&str> = schedule.split_whitespace().collect();
    if fields.len() < 2 {
        return None;
    }
    let minute: u32 = fields[0].parse().ok()?;
    let hour: u32 = fields[1].parse().ok()?;
    Some(hour * 60 + minute)
}

/// Circular distance between two times on a 24-hour ring.
fn circular_distance(a: u32, b: u32) -> u32 {
    let diff = if a > b { a - b } else { b - a };
    diff.min(MINUTES_IN_DAY - diff)
}

/// Check if a candidate minute-of-day has sufficient gap from all allocated slots.
fn has_sufficient_gap(candidate: u32, allocated: &[u32], min_gap_minutes: u32) -> bool {
    allocated
        .iter()
        .all(|&slot| circular_distance(candidate, slot) >= min_gap_minutes)
}

/// Count how many @random fields a schedule has (fewer = more constrained).
fn random_field_count(schedule: &str) -> usize {
    schedule
        .split_whitespace()
        .filter(|f| *f == "@random")
        .count()
}

/// Batch-resolve @random schedules with minimum gap enforcement.
///
/// Input: `(job_name, raw_schedule, existing_resolved)`
/// Output: `(job_name, resolved_schedule)`
///
/// Implements slot-based gap enforcement:
/// - Non-random jobs get identity resolution
/// - Random jobs sorted by constraint severity (fewer @random fields first)
/// - Feasibility pre-check with gap relaxation for overflow
/// - T-05-02: Retry capped at 100 per job; infeasible gap relaxation ensures termination
pub fn resolve_random_schedules_batch(
    jobs: &[(String, String, Option<String>)],
    min_gap: Duration,
    rng: &mut impl Rng,
) -> Vec<(String, String)> {
    let mut results: Vec<(String, String)> = Vec::with_capacity(jobs.len());
    let mut allocated_slots: Vec<u32> = Vec::new();

    // Separate random and non-random jobs.
    let mut random_jobs: Vec<(usize, &String, &String, &Option<String>)> = Vec::new();

    for (i, (name, raw, existing)) in jobs.iter().enumerate() {
        if is_random_schedule(raw) {
            random_jobs.push((i, name, raw, existing));
        } else {
            // Non-random: identity resolution.
            results.push((name.clone(), resolve_schedule(raw, existing.as_deref(), rng)));
        }
    }

    let num_random = random_jobs.len();
    let mut gap_minutes = (min_gap.as_secs() / 60) as u32;

    // Feasibility pre-check.
    if num_random > 0 && gap_minutes > 0 {
        let needed = num_random as u32 * gap_minutes;
        if needed > MINUTES_IN_DAY {
            let relaxed = MINUTES_IN_DAY / num_random as u32;
            tracing::warn!(
                target: "cronduit.random",
                jobs = num_random,
                gap_minutes = gap_minutes,
                relaxed_gap_minutes = relaxed,
                "random_min_gap is infeasible; relaxing gap for overflow jobs"
            );
            gap_minutes = relaxed;
        }
    }

    // Sort random jobs by constraint severity (fewer @random fields = more constrained = first).
    random_jobs.sort_by_key(|(_, _, raw, _)| random_field_count(raw));

    for (_idx, name, raw, existing) in &random_jobs {
        if gap_minutes == 0 {
            // No gap enforcement needed.
            let resolved = resolve_schedule(raw, existing.as_deref(), rng);
            results.push(((*name).clone(), resolved));
            continue;
        }

        // If existing resolved is provided, check if it satisfies the gap.
        if let Some(ex) = existing.as_deref() {
            if let Some(mod_val) = minute_of_day(ex) {
                if has_sufficient_gap(mod_val, &allocated_slots, gap_minutes) {
                    allocated_slots.push(mod_val);
                    results.push(((*name).clone(), ex.to_string()));
                    continue;
                }
            }
            // Existing doesn't satisfy gap; re-resolve.
        }

        // Try to find a slot that satisfies the gap constraint.
        let mut best_candidate: Option<(String, u32, u32)> = None; // (schedule, mod, min_dist)

        for _ in 0..MAX_SLOT_RETRIES {
            let candidate = resolve_schedule(raw, None, rng);
            if let Some(mod_val) = minute_of_day(&candidate) {
                if has_sufficient_gap(mod_val, &allocated_slots, gap_minutes) {
                    allocated_slots.push(mod_val);
                    results.push(((*name).clone(), candidate));
                    best_candidate = None; // signal success
                    break;
                }
                // Track the candidate with the maximum minimum distance to neighbors.
                let min_dist = allocated_slots
                    .iter()
                    .map(|&s| circular_distance(mod_val, s))
                    .min()
                    .unwrap_or(MINUTES_IN_DAY);
                if best_candidate
                    .as_ref()
                    .map_or(true, |(_s, _m, d)| min_dist > *d)
                {
                    best_candidate = Some((candidate, mod_val, min_dist));
                }
                continue;
            }
            // Couldn't extract minute-of-day; keep trying.
        }

        // If we didn't break out of the loop (no success), use best candidate.
        if let Some((sched, mod_val, min_dist)) = best_candidate {
            tracing::warn!(
                target: "cronduit.random",
                job = %name,
                min_distance_minutes = min_dist,
                requested_gap_minutes = gap_minutes,
                "could not satisfy gap constraint after {} retries; using best candidate",
                MAX_SLOT_RETRIES
            );
            allocated_slots.push(mod_val);
            results.push(((*name).clone(), sched));
        }
    }

    results
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
                let diff = circular_distance_test(minutes[i], minutes[j], 1440);
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
    fn circular_distance_test(a: u32, b: u32, modulus: u32) -> u32 {
        let diff = if a > b { a - b } else { b - a };
        diff.min(modulus - diff)
    }
}
