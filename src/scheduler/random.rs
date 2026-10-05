//! @random cron field resolver.
//!
//! Transforms schedule strings containing `@random` tokens into concrete cron values.
//! Enforces minimum spacing between @random jobs via a slot-based algorithm.
//! Spacing is day-of-week aware: two jobs only need `random_min_gap` separation
//! where their resolved schedules can fire on the same day (distances are
//! measured on a weekly ring, so 23:50 Mon vs 00:10 Tue is still 20 minutes).
//! Handles infeasibility by relaxing the gap with a warning.
//!
//! T-05-01: Validates field count before resolution; rejects malformed input.
//! T-05-02: Caps retry attempts and relaxes infeasible gaps to guarantee termination.

use rand::RngExt;
use std::str::FromStr;
use std::time::Duration;

/// Valid ranges for each of the 5 standard cron fields.
const FIELD_RANGES: [(u32, u32); 5] = [
    (0, 59), // minute
    (0, 23), // hour
    (1, 31), // day of month
    (1, 12), // month
    (0, 6),  // day of week
];

/// Minutes in a day, used for circular gap calculations.
const MINUTES_IN_DAY: u32 = 1440;

/// Minutes in a week, the ring on which resolved fire times are compared.
const MINUTES_IN_WEEK: u32 = 7 * MINUTES_IN_DAY;

/// Day-of-week bitmask with every day (Sun..Sat) set.
const ALL_DAYS: u8 = 0b0111_1111;

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
    rng: &mut impl RngExt,
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
    let last_attempt = resolve_fields(&fields, rng);
    tracing::warn!(
        target: "cronduit.random",
        schedule = %raw,
        resolved = %last_attempt,
        "failed to resolve valid cron after {} attempts, accepting unvalidated result",
        MAX_RESOLVE_RETRIES
    );
    last_attempt
}

/// Replace @random fields with random values from the appropriate range.
fn resolve_fields(fields: &[&str], rng: &mut impl RngExt) -> String {
    fields
        .iter()
        .enumerate()
        .map(|(i, &field)| {
            if field == "@random" {
                let (min, max) = FIELD_RANGES[i];
                rng.random_range(min..=max).to_string()
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

/// Days of the week (bit `d` = day `d`, Sunday = 0) on which a 5-field cron
/// schedule can fire.
///
/// Conservative by design: anything that cannot be pinned to a definite set of
/// weekdays is treated as "every day", which can only make spacing stricter.
/// That covers `*`/`?`, names, `L`/`#` modifiers, unresolved `@random`, and any
/// restricted day-of-month field (Vixie cron ORs a restricted dom with the dow,
/// so such a job can fire on any weekday).
fn dow_mask(schedule: &str) -> u8 {
    let fields: Vec<&str> = schedule.split_whitespace().collect();
    if fields.len() != 5 || !matches!(fields[2], "*" | "?") {
        return ALL_DAYS;
    }
    let mut mask = 0u8;
    for item in fields[4].split(',') {
        let (range, step) = match item.split_once('/') {
            Some((r, s)) => match s.parse::<u32>() {
                Ok(step) if step > 0 => (r, step),
                _ => return ALL_DAYS,
            },
            None => (item, 1),
        };
        let (lo, hi) = match range {
            "*" | "?" => (0, 6),
            _ => match range.split_once('-') {
                Some((a, b)) => match (a.parse::<u32>(), b.parse::<u32>()) {
                    (Ok(a), Ok(b)) if a <= b && b <= 7 => (a, b),
                    _ => return ALL_DAYS,
                },
                None => match range.parse::<u32>() {
                    // `N/step` means "from N to the end of the range".
                    Ok(v) if v <= 7 && item.contains('/') => (v, 7),
                    Ok(v) if v <= 7 => (v, v),
                    _ => return ALL_DAYS,
                },
            },
        };
        for v in (lo..=hi).step_by(step as usize) {
            // `7` is an alias for Sunday.
            mask |= 1 << (v % 7);
        }
    }
    if mask == 0 { ALL_DAYS } else { mask }
}

/// Minute-of-week fire positions of a resolved schedule: one per day it can
/// fire on. Returns `None` when the schedule has no single fire time per day
/// (e.g. hour `*`), in which case gap enforcement does not apply.
fn week_slots(schedule: &str) -> Option<Vec<u32>> {
    let mod_val = minute_of_day(schedule)?;
    let mask = dow_mask(schedule);
    Some(
        (0..7)
            .filter(|d| mask & (1 << d) != 0)
            .map(|d| d * MINUTES_IN_DAY + mod_val)
            .collect(),
    )
}

/// Circular distance between two times on a 7-day ring.
fn circular_distance(a: u32, b: u32) -> u32 {
    let diff = a.abs_diff(b);
    diff.min(MINUTES_IN_WEEK - diff)
}

/// Smallest distance between any candidate fire position and any allocated one.
fn min_distance(candidate: &[u32], allocated: &[u32]) -> u32 {
    candidate
        .iter()
        .flat_map(|&c| allocated.iter().map(move |&a| circular_distance(c, a)))
        .min()
        .unwrap_or(MINUTES_IN_WEEK)
}

/// Check if a candidate's fire positions have sufficient gap from all allocated slots.
fn has_sufficient_gap(candidate: &[u32], allocated: &[u32], min_gap_minutes: u32) -> bool {
    min_distance(candidate, allocated) >= min_gap_minutes
}

/// Whether the minute and hour fields resolve to one fixed time of day, i.e.
/// whether `random_min_gap` applies to this raw schedule at all.
fn has_single_time_of_day(raw: &str) -> bool {
    let fields: Vec<&str> = raw.split_whitespace().collect();
    fields.len() == 5
        && fields[..2]
            .iter()
            .all(|f| *f == "@random" || f.parse::<u32>().is_ok())
}

/// Smallest achievable worst-case number of gap-constrained @random jobs
/// sharing one day.
///
/// Jobs whose day-of-week is `@random` (with an unrestricted day-of-month) can
/// be placed on any single day, so they are spread over the least-loaded days;
/// every other job counts against each day in its [`dow_mask`].
fn max_jobs_per_day(raws: &[&str]) -> u32 {
    let mut fixed = [0u32; 7];
    let mut flexible = 0u32;
    for raw in raws.iter().filter(|r| has_single_time_of_day(r)) {
        let fields: Vec<&str> = raw.split_whitespace().collect();
        if fields[4] == "@random" && matches!(fields[2], "*" | "?") {
            flexible += 1;
            continue;
        }
        let mask = dow_mask(raw);
        for (d, load) in fixed.iter_mut().enumerate() {
            if mask & (1 << d) != 0 {
                *load += 1;
            }
        }
    }
    let mut level = fixed.iter().copied().max().unwrap_or(0);
    while fixed.iter().map(|&f| level - f).sum::<u32>() < flexible {
        level += 1;
    }
    level
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
/// - Gap is enforced only between jobs that can fire on the same day
///   (day-of-week aware; see [`dow_mask`])
/// - Feasibility pre-check per day bucket with gap relaxation for overflow
/// - Jobs without a single time of day (e.g. hour `*`) are exempt from the gap
/// - T-05-02: Retry capped at 100 per job; infeasible gap relaxation ensures termination
pub fn resolve_random_schedules_batch(
    jobs: &[(String, String, Option<String>)],
    min_gap: Duration,
    rng: &mut impl RngExt,
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
            results.push((
                name.clone(),
                resolve_schedule(raw, existing.as_deref(), rng),
            ));
        }
    }

    let mut gap_minutes = (min_gap.as_secs() / 60) as u32;

    // Feasibility pre-check, judged on the busiest day after spreading
    // @random day-of-week jobs as evenly as possible.
    let per_day = max_jobs_per_day(
        &random_jobs
            .iter()
            .map(|(_, _, raw, _)| raw.as_str())
            .collect::<Vec<_>>(),
    );
    if per_day > 0 && gap_minutes > 0 {
        let needed = per_day * gap_minutes;
        if needed > MINUTES_IN_DAY {
            let relaxed = MINUTES_IN_DAY / per_day;
            tracing::warn!(
                target: "cronduit.random",
                jobs = random_jobs.len(),
                jobs_per_day = per_day,
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
        if let Some(ex) = existing.as_deref()
            && let Some(slots) = week_slots(ex)
            && has_sufficient_gap(&slots, &allocated_slots, gap_minutes)
        {
            allocated_slots.extend(slots);
            results.push(((*name).clone(), ex.to_string()));
            continue;
        }
        // Existing doesn't satisfy gap or not parseable; re-resolve.

        // Try to find a slot that satisfies the gap constraint.
        let mut best_candidate: Option<(String, Vec<u32>, u32)> = None; // (schedule, slots, min_dist)

        for _ in 0..MAX_SLOT_RETRIES {
            let candidate = resolve_schedule(raw, None, rng);
            let Some(slots) = week_slots(&candidate) else {
                // No single time of day (e.g. hour `*`): the gap does not apply.
                results.push(((*name).clone(), candidate));
                best_candidate = None;
                break;
            };
            if has_sufficient_gap(&slots, &allocated_slots, gap_minutes) {
                allocated_slots.extend(slots);
                results.push(((*name).clone(), candidate));
                best_candidate = None; // signal success
                break;
            }
            // Track the candidate with the maximum minimum distance to neighbors.
            let min_dist = min_distance(&slots, &allocated_slots);
            if best_candidate
                .as_ref()
                .is_none_or(|(_s, _m, d)| min_dist > *d)
            {
                best_candidate = Some((candidate, slots, min_dist));
            }
        }

        // If we didn't break out of the loop (no success), use best candidate.
        if let Some((sched, slots, min_dist)) = best_candidate {
            tracing::warn!(
                target: "cronduit.random",
                job = %name,
                min_distance_minutes = min_dist,
                requested_gap_minutes = gap_minutes,
                "could not satisfy gap constraint after {} retries; using best candidate",
                MAX_SLOT_RETRIES
            );
            allocated_slots.extend(slots);
            results.push(((*name).clone(), sched));
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

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
        let result = resolve_schedule("@random 14 * * *", Some("42 14 * * *"), &mut rng);
        assert_eq!(result, "42 14 * * *");
    }

    #[test]
    fn stable_across_reload() {
        // Existing resolved with same raw schedule => preserved
        let mut rng = seeded_rng();
        let result = resolve_schedule("@random @random * * *", Some("15 8 * * *"), &mut rng);
        assert_eq!(
            result, "15 8 * * *",
            "should preserve existing resolved schedule"
        );
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
            .map(|i| {
                (
                    format!("job-{i}"),
                    "@random @random * * *".to_string(),
                    None,
                )
            })
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
                    i,
                    j,
                    diff
                );
            }
        }
    }

    #[test]
    fn infeasible_gap_relaxes() {
        let mut rng = seeded_rng();
        // 30 jobs * 90min = 2700min > 1440min/day => infeasible
        let jobs: Vec<(String, String, Option<String>)> = (0..30)
            .map(|i| {
                (
                    format!("job-{i}"),
                    "@random @random * * *".to_string(),
                    None,
                )
            })
            .collect();
        let min_gap = Duration::from_secs(5400); // 90 minutes

        // Should not panic -- relaxes gap instead
        let results = resolve_random_schedules_batch(&jobs, min_gap, &mut rng);
        assert_eq!(results.len(), 30);

        // All should be valid cron
        for (name, schedule) in &results {
            let cron = schedule.parse::<croner::Cron>();
            assert!(
                cron.is_ok(),
                "job {} schedule '{}' should be valid",
                name,
                schedule
            );
        }
    }

    #[test]
    fn batch_non_random_passthrough() {
        let mut rng = seeded_rng();
        let jobs = vec![
            ("fixed".to_string(), "0 14 * * *".to_string(), None),
            (
                "random".to_string(),
                "@random @random * * *".to_string(),
                None,
            ),
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

    fn random_jobs(n: usize, schedule: &str) -> Vec<(String, String, Option<String>)> {
        (0..n)
            .map(|i| (format!("job-{i}"), schedule.to_string(), None))
            .collect()
    }

    /// Smallest weekly distance between any two jobs that share a fire day.
    fn min_pairwise_gap(results: &[(String, String)]) -> u32 {
        let slots: Vec<Vec<u32>> = results
            .iter()
            .map(|(_, s)| week_slots(s).expect("resolved schedule has a time of day"))
            .collect();
        let mut min = MINUTES_IN_WEEK;
        for i in 0..slots.len() {
            for j in (i + 1)..slots.len() {
                min = min.min(min_distance(&slots[i], &slots[j]));
            }
        }
        min
    }

    #[test]
    fn dow_mask_parsing() {
        assert_eq!(dow_mask("0 3 * * *"), ALL_DAYS);
        assert_eq!(dow_mask("0 3 * * ?"), ALL_DAYS);
        assert_eq!(dow_mask("0 3 * * 2"), 0b000_0100);
        assert_eq!(dow_mask("0 3 * * 7"), 0b000_0001, "7 is Sunday");
        assert_eq!(dow_mask("0 3 * * 1-5"), 0b011_1110);
        assert_eq!(dow_mask("0 3 * * 0,6"), 0b100_0001);
        assert_eq!(dow_mask("0 3 * * */2"), 0b101_0101);
        assert_eq!(
            dow_mask("0 3 * * 1/3"),
            0b001_0010 | 0b000_0001,
            "1,4,7(=0)"
        );
        // Unpinnable weekdays are conservatively treated as every day.
        assert_eq!(dow_mask("0 3 * * MON"), ALL_DAYS);
        assert_eq!(dow_mask("0 3 * * 5L"), ALL_DAYS);
        assert_eq!(dow_mask("0 3 * * @random"), ALL_DAYS);
        // A restricted day-of-month ORs with the dow, so any day can fire.
        assert_eq!(dow_mask("0 3 15 * 2"), ALL_DAYS);
    }

    #[test]
    fn per_day_load_spreads_random_dow() {
        // Issue #76 shape: 73 jobs, each free to pick its day-of-week.
        let raws = vec!["@random @random * * @random"; 73];
        assert_eq!(max_jobs_per_day(&raws), 11); // ceil(73 / 7)
        // Wildcard dow pins every job to every day.
        let raws = vec!["@random @random * * *"; 10];
        assert_eq!(max_jobs_per_day(&raws), 10);
        // Fixed loads are filled up to before flexible jobs raise the peak.
        let mut raws = vec!["@random @random * * 1"; 5];
        raws.extend(vec!["@random @random * * @random"; 6]);
        assert_eq!(max_jobs_per_day(&raws), 5);
        // No single time of day => exempt from the gap and from the count.
        assert_eq!(max_jobs_per_day(&["@random * * * *"]), 0);
    }

    #[test]
    fn random_dow_jobs_fit_without_relaxing() {
        // Issue #76: 73 x `@random @random * * @random` with a 90m gap used to
        // be judged infeasible (73 * 90 > 1440) and still exhaust retries.
        for seed in 0..20 {
            let mut rng = StdRng::seed_from_u64(seed);
            let jobs = random_jobs(73, "@random @random * * @random");
            let results =
                resolve_random_schedules_batch(&jobs, Duration::from_secs(5400), &mut rng);
            assert_eq!(results.len(), 73);
            let gap = min_pairwise_gap(&results);
            assert!(gap >= 90, "seed {seed}: jobs only {gap} minutes apart");
        }
    }

    #[test]
    fn same_dow_jobs_respect_gap() {
        let mut rng = seeded_rng();
        let jobs = random_jobs(8, "@random @random * * 3");
        let results = resolve_random_schedules_batch(&jobs, Duration::from_secs(5400), &mut rng);
        assert_eq!(results.len(), 8);
        assert!(results.iter().all(|(_, s)| s.ends_with(" 3")));
        assert!(min_pairwise_gap(&results) >= 90);
    }

    #[test]
    fn different_dow_jobs_may_share_time_of_day() {
        // Seven jobs pinned to the same hour on distinct weekdays: 60 slots per
        // day can't hold seven 90m-apart jobs, but they never share a day.
        let mut rng = seeded_rng();
        let jobs: Vec<(String, String, Option<String>)> = (0..7)
            .map(|d| (format!("job-{d}"), format!("@random 12 * * {d}"), None))
            .collect();
        let results = resolve_random_schedules_batch(&jobs, Duration::from_secs(5400), &mut rng);
        assert_eq!(results.len(), 7);
        assert!(min_pairwise_gap(&results) >= 90);
    }

    #[test]
    fn wildcard_dow_conflicts_with_every_day() {
        // A daily job at 12:00 must keep a Tuesday-only job 90m away too, so
        // the Tuesday job's persisted 12:30 slot is rejected and re-rolled.
        let mut rng = seeded_rng();
        let jobs = vec![
            (
                "daily".to_string(),
                "@random @random * * *".to_string(),
                Some("0 12 * * *".to_string()),
            ),
            (
                "tuesday".to_string(),
                "@random @random * * 2".to_string(),
                Some("30 12 * * 2".to_string()),
            ),
        ];
        let results = resolve_random_schedules_batch(&jobs, Duration::from_secs(5400), &mut rng);
        let daily = &results.iter().find(|(n, _)| n == "daily").unwrap().1;
        let tuesday = &results.iter().find(|(n, _)| n == "tuesday").unwrap().1;
        assert_eq!(daily, "0 12 * * *");
        assert_ne!(tuesday, "30 12 * * 2", "existing slot violates the gap");
        assert!(min_pairwise_gap(&results) >= 90);
    }

    #[test]
    fn gap_wraps_across_midnight_between_days() {
        // 23:50 Monday and 00:10 Tuesday are 20 minutes apart.
        let mon = week_slots("50 23 * * 1").unwrap();
        let tue = week_slots("10 0 * * 2").unwrap();
        assert_eq!(min_distance(&mon, &tue), 20);
        // Saturday 23:50 and Sunday 00:10 wrap around the week.
        let sat = week_slots("50 23 * * 6").unwrap();
        let sun = week_slots("10 0 * * 0").unwrap();
        assert_eq!(min_distance(&sat, &sun), 20);
    }

    #[test]
    fn no_time_of_day_is_resolved_without_gap() {
        // Previously dropped from the results entirely (hour `*`).
        let mut rng = seeded_rng();
        let jobs = random_jobs(2, "@random * * * *");
        let results = resolve_random_schedules_batch(&jobs, Duration::from_secs(5400), &mut rng);
        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .all(|(_, s)| validate_cron(s) && !is_random_schedule(s))
        );
    }

    /// Helper: circular distance on a ring of `modulus` size
    fn circular_distance_test(a: u32, b: u32, modulus: u32) -> u32 {
        let diff = a.abs_diff(b);
        diff.min(modulus - diff)
    }
}
