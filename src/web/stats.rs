//! Pure-Rust percentile helper (Phase 13 OBS-04 / D-19).
//!
//! Algorithm: nearest-rank, 1-indexed. Always returns an observed sample —
//! never an interpolated value that didn't occur. Matches the percentile
//! semantics documented in `.planning/phases/13-observability-polish-rc-2/13-CONTEXT.md` § D-19.
//!
//! OBS-05 structural-parity: this module is the ONLY path by which p50/p95
//! are computed, regardless of whether the backend is SQLite or Postgres.
//! Do NOT introduce a SQL-native variant on Postgres.

/// Returns the q-th percentile of `samples` using the 1-indexed nearest-rank
/// method. `q` is a fraction in `[0.0, 1.0]`. Returns `None` for empty input.
pub fn percentile(samples: &[u64], q: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let n = samples.len();
    let mut sorted: Vec<u64> = samples.to_vec();
    sorted.sort_unstable();
    let rank = (q * n as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(n - 1);
    Some(sorted[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_slice_returns_none() {
        assert_eq!(percentile(&[], 0.5), None);
        assert_eq!(percentile(&[], 0.95), None);
        assert_eq!(percentile(&[], 0.0), None);
        assert_eq!(percentile(&[], 1.0), None);
    }

    #[test]
    fn single_element_any_quantile() {
        assert_eq!(percentile(&[42], 0.5), Some(42));
        assert_eq!(percentile(&[42], 0.95), Some(42));
        assert_eq!(percentile(&[42], 0.0), Some(42));
        assert_eq!(percentile(&[42], 1.0), Some(42));
    }

    #[test]
    fn median_of_ten_returns_fifth_sample() {
        let s = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&s, 0.5), Some(50));
    }

    #[test]
    fn p95_of_ten_returns_last_sample() {
        let s = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&s, 0.95), Some(100));
    }

    #[test]
    fn sort_internal_regardless_of_input_order() {
        let sorted = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let reverse = [100, 90, 80, 70, 60, 50, 40, 30, 20, 10];
        assert_eq!(percentile(&sorted, 0.5), percentile(&reverse, 0.5));
        assert_eq!(percentile(&sorted, 0.95), percentile(&reverse, 0.95));
    }

    #[test]
    fn q_zero_returns_min() {
        let s = [5, 1, 9, 3, 7];
        assert_eq!(percentile(&s, 0.0), Some(1));
    }

    #[test]
    fn q_one_returns_max() {
        let s = [5, 1, 9, 3, 7];
        assert_eq!(percentile(&s, 1.0), Some(9));
    }

    #[test]
    fn p50_p95_over_hundred_samples() {
        let samples: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile(&samples, 0.5), Some(50));
        assert_eq!(percentile(&samples, 0.95), Some(95));
    }
}
