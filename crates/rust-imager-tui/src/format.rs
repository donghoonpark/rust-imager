//! Human-readable TUI formatting.

use std::time::Duration;

/// Format bytes using IEC units.
#[must_use]
pub fn bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut divisor = 1_u64;
    let mut unit = 0;
    while value / divisor >= 1024 && unit < UNITS.len() - 1 {
        divisor = divisor.saturating_mul(1024);
        unit += 1;
    }
    if unit == 0 {
        format!("{value} {}", UNITS[unit])
    } else {
        let tenths = u128::from(value).saturating_mul(10) / u128::from(divisor);
        format!("{}.{:01} {}", tenths / 10, tenths % 10, UNITS[unit])
    }
}

/// Format a byte rate.
#[must_use]
pub fn speed(value: u64) -> String {
    format!("{}/s", bytes(value))
}

/// Format a duration compactly.
#[must_use]
pub fn duration(value: Duration) -> String {
    let seconds = value.as_secs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Format a progress percentage.
#[must_use]
pub fn percentage(bytes: u64, total: u64) -> String {
    if total == 0 {
        return "0.0%".into();
    }
    let tenths = (u128::from(bytes.min(total)) * 1000) / u128::from(total);
    format!("{}.{:01}%", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::{bytes, duration, percentage, speed};
    use std::time::Duration;

    #[test]
    fn formats_dashboard_values() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1_048_576), "1.0 MiB");
        assert_eq!(speed(1_048_576), "1.0 MiB/s");
        assert_eq!(duration(Duration::from_secs(65)), "01:05");
        assert_eq!(duration(Duration::from_secs(3661)), "01:01:01");
        assert_eq!(percentage(1, 4), "25.0%");
    }
}
