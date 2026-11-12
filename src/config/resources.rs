//! Docker resource-limit parsing (`memory`, `cpus`) for docker jobs.

/// Parse a human memory size (`"16g"`, `"512m"`, `"1.5gb"`, `"1048576"`) into bytes.
///
/// Units are binary (1k = 1024 bytes), matching Docker / Compose `mem_limit`.
/// A bare number is bytes. Rejects empty, non-positive, and unparsable input.
pub fn parse_memory_bytes(input: &str) -> Result<i64, String> {
    let s = input.trim().to_ascii_lowercase();
    if s.is_empty() {
        return Err("memory must not be empty".to_string());
    }
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let value: f64 = num.parse().map_err(|_| {
        format!("`{input}` is not a valid memory size (expected e.g. \"512m\", \"16g\")")
    })?;
    let mult: f64 = match unit.trim() {
        "" | "b" => 1.0,
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" | "tib" => 1024.0_f64.powi(4),
        other => {
            return Err(format!(
                "`{input}` has unknown unit `{other}` (use b, k, m, g, or t)"
            ));
        }
    };
    let bytes = value * mult;
    if !bytes.is_finite() || bytes < 1.0 || bytes > i64::MAX as f64 {
        return Err(format!("`{input}` must be a positive size"));
    }
    let bytes = bytes as i64;
    if bytes < MIN_MEMORY_BYTES {
        return Err(format!(
            "`{input}` is below the 6 MiB minimum memory limit Docker accepts ({MIN_MEMORY_BYTES} bytes)"
        ));
    }
    Ok(bytes)
}

/// Docker rejects container memory limits below 6 MiB at create time.
pub const MIN_MEMORY_BYTES: i64 = 6 * 1024 * 1024;
/// Docker rejects CPU limits below 0.01 CPUs (10_000_000 nano_cpus) at create time.
pub const MIN_CPUS: f64 = 0.01;

/// Convert a CPU count (`8`, `1.5`) into Docker `nano_cpus`.
pub fn cpus_to_nano(cpus: f64) -> Result<i64, String> {
    if !cpus.is_finite() || cpus <= 0.0 {
        return Err(format!("`cpus` must be a positive number (got {cpus})"));
    }
    if cpus < MIN_CPUS {
        return Err(format!(
            "`cpus` must be at least {MIN_CPUS} (Docker rejects smaller limits; got {cpus})"
        ));
    }
    let nano = (cpus * 1e9).round();
    if nano < 1.0 || nano > i64::MAX as f64 {
        return Err(format!("`cpus` value {cpus} is out of range"));
    }
    Ok(nano as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_units() {
        assert_eq!(parse_memory_bytes("16g").unwrap(), 16 * 1024 * 1024 * 1024);
        assert_eq!(parse_memory_bytes("512M").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_memory_bytes("1.5gb").unwrap(), 1536 * 1024 * 1024);
        assert_eq!(parse_memory_bytes("6291456").unwrap(), 6 * 1024 * 1024);
        assert_eq!(parse_memory_bytes(" 8m ").unwrap(), 8 * 1024 * 1024);
    }

    #[test]
    fn memory_rejects_bad() {
        for bad in ["", "g", "0", "0m", "-1g", "12x", "abc", "1..5g"] {
            assert!(parse_memory_bytes(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn memory_below_docker_minimum_rejected() {
        for bad in ["4096", "64k", "5m", "6291455"] {
            let e = parse_memory_bytes(bad).unwrap_err();
            assert!(e.contains("6 MiB"), "{bad}: {e}");
        }
    }

    #[test]
    fn cpus_below_docker_minimum_rejected() {
        assert!(cpus_to_nano(0.009).unwrap_err().contains("at least 0.01"));
        assert_eq!(cpus_to_nano(0.01).unwrap(), 10_000_000);
    }

    #[test]
    fn cpus_nano() {
        assert_eq!(cpus_to_nano(8.0).unwrap(), 8_000_000_000);
        assert_eq!(cpus_to_nano(0.5).unwrap(), 500_000_000);
        assert!(cpus_to_nano(0.0).is_err());
        assert!(cpus_to_nano(-1.0).is_err());
        assert!(cpus_to_nano(f64::NAN).is_err());
    }
}
