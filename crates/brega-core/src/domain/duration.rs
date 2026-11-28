//! Human-entered durations in whole minutes: task effort estimates (R-10)
//! and area weekly budgets (R-11).
//!
//! Accepted forms are `90m`, `2h`, `1h30m`, and `1h 30m` (case-insensitive,
//! surrounding whitespace ignored). A bare number is rejected because its
//! unit would be a guess.

/// Largest accepted duration: 1000 hours.
pub const MAX_MINUTES: u32 = 60_000;

/// Why a duration string was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DurationError {
    /// Empty input.
    #[error("duration is empty")]
    Empty,
    /// Not of the form `90m`, `2h`, or `1h30m`.
    #[error("expected a duration like 90m, 2h, or 1h30m")]
    Malformed,
    /// Zero total duration.
    #[error("duration must be greater than zero")]
    Zero,
    /// Larger than [`MAX_MINUTES`].
    #[error("duration must be at most 1000h")]
    TooLarge,
}

/// Parse a duration into whole minutes.
///
/// # Errors
/// [`DurationError`] for empty, malformed, zero, or too-large input.
pub fn parse_minutes(input: &str) -> Result<u32, DurationError> {
    let s = input.trim().to_ascii_lowercase();
    if s.is_empty() {
        return Err(DurationError::Empty);
    }
    let (hours, rest) = match s.split_once('h') {
        Some((h, rest)) => (Some(parse_number(h)?), rest.trim_start()),
        None => (None, s.as_str()),
    };
    let minutes = if rest.is_empty() {
        None
    } else {
        let m = rest.strip_suffix('m').ok_or(DurationError::Malformed)?;
        let m = parse_number(m)?;
        if hours.is_some() && m >= 60 {
            return Err(DurationError::Malformed);
        }
        Some(m)
    };
    if hours.is_none() && minutes.is_none() {
        return Err(DurationError::Malformed);
    }
    let total = hours
        .unwrap_or(0)
        .checked_mul(60)
        .and_then(|h| h.checked_add(minutes.unwrap_or(0)))
        .ok_or(DurationError::TooLarge)?;
    match total {
        0 => Err(DurationError::Zero),
        t if t > u64::from(MAX_MINUTES) => Err(DurationError::TooLarge),
        // Bounded by MAX_MINUTES above.
        t => u32::try_from(t).map_err(|_| DurationError::TooLarge),
    }
}

fn parse_number(s: &str) -> Result<u64, DurationError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DurationError::Malformed);
    }
    s.parse::<u64>().map_err(|_| DurationError::TooLarge)
}

/// Format whole minutes as `45m`, `2h`, or `1h30m`.
#[must_use]
pub fn format_minutes(minutes: u32) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h{m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_forms() {
        for (input, want) in [
            ("90m", 90),
            ("2h", 120),
            ("1h30m", 90),
            ("1h 30m", 90),
            (" 1H30M ", 90),
            ("0h45m", 45),
            ("1h0m", 60),
            ("1000h", MAX_MINUTES),
        ] {
            assert_eq!(parse_minutes(input), Ok(want), "{input}");
        }
    }

    #[test]
    fn rejects_invalid_forms() {
        use DurationError::{Empty, Malformed, TooLarge, Zero};
        for (input, want) in [
            ("", Empty),
            ("   ", Empty),
            ("30", Malformed),
            ("-5m", Malformed),
            ("1.5h", Malformed),
            ("h", Malformed),
            ("m", Malformed),
            ("1h75m", Malformed),
            ("1h2h", Malformed),
            ("30m1h", Malformed),
            ("1d", Malformed),
            ("1 h", Malformed),
            ("0m", Zero),
            ("0h0m", Zero),
            ("1001h", TooLarge),
            ("60001m", TooLarge),
            ("99999999999999999999m", TooLarge),
        ] {
            assert_eq!(parse_minutes(input), Err(want), "{input:?}");
        }
    }

    #[test]
    fn format_round_trips() {
        for m in [1, 45, 59, 60, 90, 120, 61, MAX_MINUTES] {
            assert_eq!(parse_minutes(&format_minutes(m)), Ok(m));
        }
        assert_eq!(format_minutes(90), "1h30m");
        assert_eq!(format_minutes(45), "45m");
        assert_eq!(format_minutes(120), "2h");
    }
}
