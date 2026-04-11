//! Shared formatting helpers for web view models.

/// Format duration in milliseconds to human-readable string.
pub fn format_duration_ms(ms: Option<i64>) -> String {
    match ms {
        Some(ms) if ms < 1000 => format!("{ms}ms"),
        Some(ms) if ms < 60_000 => format!("{:.1}s", ms as f64 / 1000.0),
        Some(ms) if ms < 3_600_000 => {
            let mins = ms / 60_000;
            let secs = (ms % 60_000) / 1000;
            format!("{mins}m {secs}s")
        }
        Some(ms) => {
            let hours = ms / 3_600_000;
            let mins = (ms % 3_600_000) / 60_000;
            format!("{hours}h {mins}m")
        }
        None => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(Some(500)), "500ms");
        assert_eq!(format_duration_ms(Some(1200)), "1.2s");
        assert_eq!(format_duration_ms(Some(135_000)), "2m 15s");
        assert_eq!(format_duration_ms(Some(7_260_000)), "2h 1m");
        assert_eq!(format_duration_ms(None), "-");
    }
}
