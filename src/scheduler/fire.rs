//! BinaryHeap-based fire queue for job scheduling.
//!
//! D-02: Uses `BinaryHeap<Reverse<FireEntry>>` for O(log n) next-fire tracking.
//! D-03: Clock-jump detection with catch-up fire enumeration.
//! D-07: All schedule evaluation uses `[server].timezone` only.

use crate::db::queries::DbJob;
use chrono::{DateTime, SubsecRound, Utc};
use chrono_tz::Tz;
use croner::Cron;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::str::FromStr;
use std::time::Duration;

/// An entry in the fire queue representing a job's next scheduled fire time.
#[derive(Debug, Clone)]
pub struct FireEntry {
    pub job_id: i64,
    pub job_name: String,
    pub fire_time: DateTime<Tz>,
    pub instant: tokio::time::Instant,
    pub resolved_schedule: String,
}

impl Eq for FireEntry {}

impl PartialEq for FireEntry {
    fn eq(&self, other: &Self) -> bool {
        self.instant == other.instant
    }
}

impl Ord for FireEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.instant.cmp(&other.instant)
    }
}

impl PartialOrd for FireEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A fire that was missed due to a clock jump.
#[derive(Debug, Clone)]
pub struct MissedFire {
    pub job_id: i64,
    pub job_name: String,
    pub missed_time: DateTime<Tz>,
}

/// Build the initial fire queue from enabled jobs.
///
/// For each job, parses the cron expression and finds the next occurrence
/// after the current time in the configured timezone.
pub fn build_initial_heap(jobs: &[DbJob], tz: Tz) -> BinaryHeap<Reverse<FireEntry>> {
    build_initial_heap_at(
        jobs,
        Utc::now().with_timezone(&tz),
        tokio::time::Instant::now(),
    )
}

/// Build the fire queue relative to an explicit wall-clock / monotonic pair.
///
/// `now_tz` and `now_instant` must describe the same moment; each entry's
/// `instant` is `now_instant` plus the wall-clock distance to its fire time.
pub fn build_initial_heap_at(
    jobs: &[DbJob],
    now_tz: DateTime<Tz>,
    now_instant: tokio::time::Instant,
) -> BinaryHeap<Reverse<FireEntry>> {
    let mut heap = BinaryHeap::new();

    for job in jobs {
        match Cron::from_str(&job.resolved_schedule) {
            Ok(cron) => match cron.find_next_occurrence(&now_tz, false) {
                Ok(next) => {
                    let until_fire = next
                        .signed_duration_since(now_tz)
                        .to_std()
                        .unwrap_or(Duration::ZERO);
                    let instant = now_instant + until_fire;

                    tracing::debug!(
                        target: "cronduit.scheduler",
                        job = %job.name,
                        next_fire = %next,
                        "scheduled next fire"
                    );

                    heap.push(Reverse(FireEntry {
                        job_id: job.id,
                        job_name: job.name.clone(),
                        fire_time: next,
                        instant,
                        resolved_schedule: job.resolved_schedule.clone(),
                    }));
                }
                Err(e) => {
                    tracing::warn!(
                        target: "cronduit.scheduler",
                        job = %job.name,
                        error = %e,
                        "failed to find next occurrence, skipping job"
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    target: "cronduit.scheduler",
                    job = %job.name,
                    schedule = %job.resolved_schedule,
                    error = %e,
                    "invalid cron expression, skipping job"
                );
            }
        }
    }

    heap
}

/// Requeue a job with its next fire time after the given reference time.
pub fn requeue_job(
    heap: &mut BinaryHeap<Reverse<FireEntry>>,
    job: &DbJob,
    after: &DateTime<Tz>,
    tz: Tz,
) {
    requeue_job_at(
        heap,
        job,
        after,
        Utc::now().with_timezone(&tz),
        tokio::time::Instant::now(),
    );
}

/// Requeue a job relative to an explicit wall-clock / monotonic pair.
///
/// See [`build_initial_heap_at`] for the meaning of `now_tz` / `now_instant`.
pub fn requeue_job_at(
    heap: &mut BinaryHeap<Reverse<FireEntry>>,
    job: &DbJob,
    after: &DateTime<Tz>,
    now_tz: DateTime<Tz>,
    now_instant: tokio::time::Instant,
) {
    let cron = match Cron::from_str(&job.resolved_schedule) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                target: "cronduit.scheduler",
                job = %job.name,
                error = %e,
                "failed to parse cron for requeue"
            );
            return;
        }
    };

    match cron.find_next_occurrence(after, false) {
        Ok(next) => {
            let until_fire = next
                .signed_duration_since(now_tz)
                .to_std()
                .unwrap_or(Duration::ZERO);
            let instant = now_instant + until_fire;

            heap.push(Reverse(FireEntry {
                job_id: job.id,
                job_name: job.name.clone(),
                fire_time: next,
                instant,
                resolved_schedule: job.resolved_schedule.clone(),
            }));
        }
        Err(e) => {
            tracing::warn!(
                target: "cronduit.scheduler",
                job = %job.name,
                error = %e,
                "failed to find next occurrence for requeue"
            );
        }
    }
}

/// Pop all entries from the heap whose fire instant is at or before `now`.
pub fn fire_due_jobs(
    heap: &mut BinaryHeap<Reverse<FireEntry>>,
    now: tokio::time::Instant,
) -> Vec<FireEntry> {
    let mut due = Vec::new();
    while let Some(top) = heap.peek() {
        if top.0.instant <= now {
            due.push(heap.pop().unwrap().0);
        } else {
            break;
        }
    }
    due
}

/// Maximum clock jump window to scan for missed fires (T-02-02: DoS mitigation).
const MAX_CATCHUP_WINDOW_HOURS: i64 = 24;

/// Wake drift beyond which a wake is treated as a wall-clock jump (D-03).
const CLOCK_JUMP_THRESHOLD_MINUTES: i64 = 2;

/// Wall-clock time at which a sleep until `sleep_target` is expected to end.
///
/// Must be computed for the sleep that is about to happen: comparing a wake
/// against an earlier sleep's target would count the whole idle gap between
/// fires as clock drift (cronduit-53x).
pub fn expected_wake(
    sleep_target: tokio::time::Instant,
    now_instant: tokio::time::Instant,
    now_tz: DateTime<Tz>,
) -> DateTime<Tz> {
    let sleep_duration = sleep_target.saturating_duration_since(now_instant);
    now_tz + chrono::Duration::from_std(sleep_duration).unwrap_or(chrono::Duration::zero())
}

/// Whether `actual_now` overshoots `expected_wake` by more than the threshold.
pub fn is_clock_jump(expected_wake: DateTime<Tz>, actual_now: DateTime<Tz>) -> bool {
    actual_now.signed_duration_since(expected_wake)
        > chrono::Duration::minutes(CLOCK_JUMP_THRESHOLD_MINUTES)
}

/// Everything the scheduler loop must spawn after the fire-queue sleep wakes.
#[derive(Debug)]
pub struct WakePlan {
    /// Entries due at this wake (`trigger = "scheduled"`).
    pub due: Vec<FireEntry>,
    /// Fires skipped by a forward clock jump (`trigger = "catch-up"`).
    pub missed: Vec<MissedFire>,
}

/// Resolve one fire-queue wake into scheduled and catch-up fires.
///
/// Pops the due entries, enumerates missed fires when the wake overshot
/// `expected_wake` (the target of *this* sleep), drops catch-up fires for a
/// slot that is already due so each slot runs at most once, then requeues.
/// After a clock jump the remaining heap instants are stale (they were
/// computed against the pre-jump wall clock), so the heap is rebuilt from
/// `actual_now`; fires in between are covered by the catch-up runs.
pub fn plan_wake(
    heap: &mut BinaryHeap<Reverse<FireEntry>>,
    jobs: &[DbJob],
    expected_wake: DateTime<Tz>,
    actual_now: DateTime<Tz>,
    now_instant: tokio::time::Instant,
) -> WakePlan {
    let due = fire_due_jobs(heap, now_instant);

    let mut missed = check_clock_jump(expected_wake, actual_now, actual_now.timezone(), jobs);
    missed.retain(|m| {
        !due.iter().any(|d| {
            d.job_id == m.job_id && d.fire_time.trunc_subsecs(0) == m.missed_time.trunc_subsecs(0)
        })
    });

    if is_clock_jump(expected_wake, actual_now) {
        *heap = build_initial_heap_at(jobs, actual_now, now_instant);
    } else {
        for entry in &due {
            if let Some(job) = jobs.iter().find(|j| j.id == entry.job_id) {
                requeue_job_at(heap, job, &entry.fire_time, actual_now, now_instant);
            }
        }
    }

    WakePlan { due, missed }
}

/// Check for clock jumps and enumerate missed fires.
///
/// D-03: If actual_now - expected_wake > 2 minutes, scan all jobs for
/// fire times in the skipped interval.
///
/// T-02-02: Limits catch-up enumeration to 24 hours to prevent unbounded
/// iteration after long hibernation.
pub fn check_clock_jump(
    expected_wake: DateTime<Tz>,
    actual_now: DateTime<Tz>,
    _tz: Tz,
    jobs: &[DbJob],
) -> Vec<MissedFire> {
    if !is_clock_jump(expected_wake, actual_now) {
        return Vec::new();
    }
    let diff = actual_now.signed_duration_since(expected_wake);

    tracing::warn!(
        target: "cronduit.scheduler",
        expected = %expected_wake,
        actual = %actual_now,
        drift_secs = diff.num_seconds(),
        "clock jump detected"
    );

    // T-02-02: Cap the scan window to prevent DoS after long hibernation.
    let scan_end = if diff > chrono::Duration::hours(MAX_CATCHUP_WINDOW_HOURS) {
        tracing::warn!(
            target: "cronduit.scheduler",
            max_hours = MAX_CATCHUP_WINDOW_HOURS,
            "clock jump exceeds maximum catch-up window, limiting scan"
        );
        expected_wake + chrono::Duration::hours(MAX_CATCHUP_WINDOW_HOURS)
    } else {
        actual_now
    };

    let mut missed = Vec::new();

    for job in jobs {
        let cron = match Cron::from_str(&job.resolved_schedule) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Iterate occurrences after expected_wake, collecting those before scan_end.
        for occurrence in cron.clone().iter_after(expected_wake) {
            if occurrence >= scan_end {
                break;
            }
            tracing::warn!(
                target: "cronduit.scheduler",
                job = %job.name,
                missed_time = %occurrence,
                "missed fire due to clock jump"
            );
            missed.push(MissedFire {
                job_id: job.id,
                job_name: job.name.clone(),
                missed_time: occurrence,
            });
        }
    }

    missed
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike};

    fn make_db_job(id: i64, name: &str, schedule: &str) -> DbJob {
        DbJob {
            id,
            name: name.to_string(),
            schedule: schedule.to_string(),
            resolved_schedule: schedule.to_string(),
            job_type: "command".to_string(),
            config_json: "{}".to_string(),
            config_hash: "test".to_string(),
            enabled: true,
            enabled_override: None,
            timeout_secs: 3600,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn dst_spring_forward_skips_nonexistent_time() {
        // America/New_York 2026-03-08: clocks spring forward at 02:00 -> 03:00.
        // A job at "30 2 * * *" targets 02:30 which doesn't exist on this day.
        // croner handles this by still returning March 8 (the wall-clock time
        // is adjusted to the post-DST offset). The important thing is that
        // the fire time is valid and on/after the reference time.
        let tz: Tz = "America/New_York".parse().unwrap();
        let before_spring = tz.with_ymd_and_hms(2026, 3, 8, 1, 59, 0).unwrap();

        let cron = Cron::from_str("30 2 * * *").unwrap();
        let next = cron.find_next_occurrence(&before_spring, false).unwrap();

        // croner returns a valid time on March 8 (DST-shifted).
        // The fire is on March 8, and it's after the reference time.
        assert_eq!(next.day(), 8);
        assert!(
            next > before_spring,
            "next fire must be after reference time"
        );

        // The next occurrence after that should be March 9 at 02:30 EST.
        let after = cron.find_next_occurrence(&next, false).unwrap();
        assert_eq!(after.day(), 9);
        assert_eq!(after.hour(), 2);
        assert_eq!(after.minute(), 30);
    }

    #[test]
    fn dst_fall_back_fires_once() {
        // America/New_York 2026-11-01: clocks fall back at 02:00 -> 01:00.
        // A job at "30 1 * * *" should fire once, at the first 01:30.
        let tz: Tz = "America/New_York".parse().unwrap();
        let before_fall = tz.with_ymd_and_hms(2026, 11, 1, 0, 59, 0).unwrap();

        let cron = Cron::from_str("30 1 * * *").unwrap();
        let next = cron.find_next_occurrence(&before_fall, false).unwrap();

        // Should fire at 01:30 on the same day.
        assert_eq!(next.day(), 1);
        assert_eq!(next.hour(), 1);
        assert_eq!(next.minute(), 30);

        // Should NOT fire twice: the next occurrence after the first 01:30
        // should be the next day.
        let after_first = cron.find_next_occurrence(&next, false).unwrap();
        assert_eq!(after_first.day(), 2);
    }

    #[test]
    fn clock_jump_detects_missed_fires() {
        // Expected wake at 12:00, actual at 12:05 (> 2 min threshold).
        // Job at "*/1 * * * *" should have missed fires at 12:01, 12:02, 12:03, 12:04.
        let tz: Tz = "UTC".parse().unwrap();
        let expected = tz.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
        let actual = tz.with_ymd_and_hms(2026, 6, 15, 12, 5, 0).unwrap();

        let jobs = vec![make_db_job(1, "every-min", "*/1 * * * *")];
        let missed = check_clock_jump(expected, actual, tz, &jobs);

        // Should find fires at 12:01, 12:02, 12:03, 12:04 (not 12:05 since it's at scan_end).
        assert_eq!(missed.len(), 4);
        assert_eq!(missed[0].missed_time.minute(), 1);
        assert_eq!(missed[1].missed_time.minute(), 2);
        assert_eq!(missed[2].missed_time.minute(), 3);
        assert_eq!(missed[3].missed_time.minute(), 4);
    }

    #[test]
    fn clock_jump_no_false_positive() {
        // Expected wake at 12:00, actual at 12:01 (< 2 min threshold).
        // Should NOT trigger catch-up.
        let tz: Tz = "UTC".parse().unwrap();
        let expected = tz.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
        let actual = tz.with_ymd_and_hms(2026, 6, 15, 12, 1, 0).unwrap();

        let jobs = vec![make_db_job(1, "every-min", "*/1 * * * *")];
        let missed = check_clock_jump(expected, actual, tz, &jobs);

        assert!(missed.is_empty());
    }

    #[test]
    fn heap_ordering_pops_earliest_first() {
        let now = tokio::time::Instant::now();
        let mut heap: BinaryHeap<Reverse<FireEntry>> = BinaryHeap::new();
        let tz: Tz = "UTC".parse().unwrap();

        // Insert entries with different fire times.
        let times = [
            ("late", Duration::from_secs(300)),
            ("early", Duration::from_secs(60)),
            ("mid", Duration::from_secs(180)),
        ];

        for (name, offset) in &times {
            heap.push(Reverse(FireEntry {
                job_id: 1,
                job_name: name.to_string(),
                fire_time: Utc::now().with_timezone(&tz),
                instant: now + *offset,
                resolved_schedule: "* * * * *".to_string(),
            }));
        }

        // Pop should yield: early (60s), mid (180s), late (300s).
        let first = heap.pop().unwrap().0;
        assert_eq!(first.job_name, "early");
        let second = heap.pop().unwrap().0;
        assert_eq!(second.job_name, "mid");
        let third = heap.pop().unwrap().0;
        assert_eq!(third.job_name, "late");
    }

    #[test]
    fn fire_due_jobs_pops_only_due() {
        let now = tokio::time::Instant::now();
        let mut heap: BinaryHeap<Reverse<FireEntry>> = BinaryHeap::new();
        let tz: Tz = "UTC".parse().unwrap();

        // One entry in the past, one in the future.
        heap.push(Reverse(FireEntry {
            job_id: 1,
            job_name: "past".to_string(),
            fire_time: Utc::now().with_timezone(&tz),
            instant: now - Duration::from_secs(10),
            resolved_schedule: "* * * * *".to_string(),
        }));
        heap.push(Reverse(FireEntry {
            job_id: 2,
            job_name: "future".to_string(),
            fire_time: Utc::now().with_timezone(&tz),
            instant: now + Duration::from_secs(300),
            resolved_schedule: "* * * * *".to_string(),
        }));

        let due = fire_due_jobs(&mut heap, now);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].job_name, "past");
        assert_eq!(heap.len(), 1); // "future" still in heap
    }

    /// Drive the scheduler loop's fire path (`heap.peek` -> `expected_wake` ->
    /// sleep -> `plan_wake`) against a simulated wall clock, exactly as
    /// `SchedulerLoop::run` wires it. `wall_at` maps a monotonic instant to
    /// the wall clock (a forward jump is an offset applied past some instant).
    /// Returns every (job_id, slot, trigger) the loop would spawn.
    fn simulate_loop(
        jobs: &[DbJob],
        start_instant: tokio::time::Instant,
        wall_at: impl Fn(tokio::time::Instant) -> DateTime<Tz>,
        wakes: usize,
    ) -> Vec<(i64, DateTime<Tz>, &'static str)> {
        let mut heap = build_initial_heap_at(jobs, wall_at(start_instant), start_instant);
        let mut now_instant = start_instant;
        let mut spawned = Vec::new();

        for _ in 0..wakes {
            let sleep_target = heap.peek().map(|r| r.0.instant).unwrap();
            let expected = expected_wake(sleep_target, now_instant, wall_at(now_instant));
            // The monotonic sleep wakes on target; only the wall clock may jump.
            now_instant = sleep_target;
            let plan = plan_wake(&mut heap, jobs, expected, wall_at(now_instant), now_instant);
            for m in plan.missed {
                spawned.push((m.job_id, m.missed_time, "catch-up"));
            }
            for d in plan.due {
                spawned.push((d.job_id, d.fire_time, "scheduled"));
            }
        }
        spawned
    }

    fn assert_each_slot_once(spawned: &[(i64, DateTime<Tz>, &'static str)]) {
        let mut slots: Vec<(i64, DateTime<Tz>)> = spawned
            .iter()
            .map(|(id, t, _)| (*id, t.trunc_subsecs(0)))
            .collect();
        let total = slots.len();
        slots.sort();
        slots.dedup();
        assert_eq!(
            slots.len(),
            total,
            "a slot was spawned more than once: {spawned:?}"
        );
    }

    #[test]
    fn idle_gaps_between_fires_are_not_clock_jumps() {
        // cronduit-53x acceptance 1 + 4: hours-long idle gaps between fires on
        // an undisturbed clock must produce no catch-up runs. The old wiring
        // compared each wake against the PREVIOUS sleep's target, so the
        // whole gap counted as drift and the due slot also fired as catch-up.
        let tz: Tz = "UTC".parse().unwrap();
        let base = tz.with_ymd_and_hms(2026, 6, 15, 0, 0, 30).unwrap();
        let start = tokio::time::Instant::now();
        let wall_at =
            |i: tokio::time::Instant| base + chrono::Duration::from_std(i - start).unwrap();
        let jobs = vec![
            make_db_job(1, "two-hourly", "0 */2 * * *"),
            make_db_job(2, "daily", "8 0 * * *"),
        ];

        let spawned = simulate_loop(&jobs, start, wall_at, 12);

        assert_eq!(spawned.len(), 12);
        assert!(
            spawned
                .iter()
                .all(|(_, _, trigger)| *trigger == "scheduled"),
            "idle gaps must not trigger catch-up: {spawned:?}"
        );
        assert_each_slot_once(&spawned);

        // Sanity: the gaps really are far beyond the jump threshold, so the
        // previous-wake comparison would have flagged them.
        let first = spawned[0].1;
        let second = spawned[1].1;
        assert!(is_clock_jump(first, second));
    }

    #[test]
    fn forward_clock_jump_catches_up_each_missed_slot_once() {
        // cronduit-53x acceptance 2 + 3 + 4: a real +1h wall-clock jump during
        // a sleep still catches up the skipped slots; the slot due at the wake
        // fires once as scheduled, and the stale heap does not refire slots the
        // catch-up already covered.
        let tz: Tz = "UTC".parse().unwrap();
        let base = tz.with_ymd_and_hms(2026, 6, 15, 12, 0, 30).unwrap();
        let start = tokio::time::Instant::now();
        let jump_at = start + Duration::from_secs(300);
        let wall_at = |i: tokio::time::Instant| {
            let jump = if i >= jump_at {
                chrono::Duration::hours(1)
            } else {
                chrono::Duration::zero()
            };
            base + chrono::Duration::from_std(i - start).unwrap() + jump
        };
        let jobs = vec![make_db_job(1, "ten-min", "*/10 * * * *")];

        let spawned = simulate_loop(&jobs, start, wall_at, 3);

        assert_each_slot_once(&spawned);
        let slot = |h, m| tz.with_ymd_and_hms(2026, 6, 15, h, m, 0).unwrap();
        let catch_up: Vec<_> = spawned
            .iter()
            .filter(|(_, _, t)| *t == "catch-up")
            .map(|(_, t, _)| *t)
            .collect();
        assert_eq!(
            catch_up,
            vec![
                slot(12, 20),
                slot(12, 30),
                slot(12, 40),
                slot(12, 50),
                slot(13, 0)
            ]
        );
        let scheduled: Vec<_> = spawned
            .iter()
            .filter(|(_, _, t)| *t == "scheduled")
            .map(|(_, t, _)| *t)
            .collect();
        // 12:10 was due at the wake (on the pre-jump timeline); after the
        // jump the heap is rebuilt from the real wall clock.
        assert_eq!(scheduled, vec![slot(12, 10), slot(13, 20), slot(13, 30)]);
    }

    #[test]
    fn plan_wake_does_not_double_fire_slot_due_at_wake() {
        // cronduit-53x acceptance 3: the wake lands exactly on a fire time and
        // the expected wake sits microseconds before it (the production log
        // showed missed_time 00:08:00.377373 vs wake 00:08:00.377745). Even
        // with a jump in progress the due slot must run once, as scheduled.
        let tz: Tz = "UTC".parse().unwrap();
        let fire_time = tz.with_ymd_and_hms(2026, 6, 15, 0, 8, 0).unwrap();
        let now_instant = tokio::time::Instant::now();
        let mut heap: BinaryHeap<Reverse<FireEntry>> = BinaryHeap::new();
        heap.push(Reverse(FireEntry {
            job_id: 1,
            job_name: "every-min".to_string(),
            fire_time,
            instant: now_instant,
            resolved_schedule: "* * * * *".to_string(),
        }));
        let jobs = vec![make_db_job(1, "every-min", "* * * * *")];
        let expected = fire_time - chrono::Duration::microseconds(372);
        let actual = fire_time + chrono::Duration::minutes(3);

        let plan = plan_wake(&mut heap, &jobs, expected, actual, now_instant);

        assert_eq!(plan.due.len(), 1);
        assert_eq!(plan.due[0].fire_time, fire_time);
        let missed: Vec<_> = plan.missed.iter().map(|m| m.missed_time.minute()).collect();
        assert_eq!(missed, vec![9, 10]);
        // Heap rebuilt after the jump: next fire is after `actual`.
        assert_eq!(heap.len(), 1);
        assert!(heap.peek().unwrap().0.fire_time > actual);
    }

    #[test]
    fn plan_wake_on_time_requeues_without_catch_up() {
        // Normal wake: no catch-up, due job requeued from its fire time.
        let tz: Tz = "UTC".parse().unwrap();
        let fire_time = tz.with_ymd_and_hms(2026, 6, 15, 0, 8, 0).unwrap();
        let now_instant = tokio::time::Instant::now();
        let mut heap: BinaryHeap<Reverse<FireEntry>> = BinaryHeap::new();
        heap.push(Reverse(FireEntry {
            job_id: 1,
            job_name: "every-min".to_string(),
            fire_time,
            instant: now_instant,
            resolved_schedule: "* * * * *".to_string(),
        }));
        let jobs = vec![make_db_job(1, "every-min", "* * * * *")];

        let plan = plan_wake(&mut heap, &jobs, fire_time, fire_time, now_instant);

        assert_eq!(plan.due.len(), 1);
        assert!(plan.missed.is_empty());
        let next = heap.peek().unwrap().0.clone();
        assert_eq!(next.fire_time, fire_time + chrono::Duration::minutes(1));
        assert_eq!(next.instant, now_instant + Duration::from_secs(60));
    }

    #[test]
    fn clock_jump_limited_to_24h_window() {
        // T-02-02: Verify catch-up is limited to 24 hours.
        let tz: Tz = "UTC".parse().unwrap();
        let expected = tz.with_ymd_and_hms(2026, 6, 15, 0, 0, 0).unwrap();
        // 48-hour jump.
        let actual = tz.with_ymd_and_hms(2026, 6, 17, 0, 0, 0).unwrap();

        let jobs = vec![make_db_job(1, "hourly", "0 * * * *")];
        let missed = check_clock_jump(expected, actual, tz, &jobs);

        // Should only find fires in the first 24 hours (0:00 to 23:00 = 23 fires
        // since iter_after excludes the start and we stop before scan_end).
        assert!(missed.len() <= 24);
        assert!(missed.len() >= 23);
    }
}
