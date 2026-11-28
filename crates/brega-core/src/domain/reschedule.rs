//! Append-only reschedule log: one record each time a task's planned day
//! (`scheduled_for`) or due date (`deadline`) moves to a later date. Input
//! to the Weekly Review "slipped" count (R-12).

use time::{Date, Month, OffsetDateTime};
use uuid::Uuid;

/// Which task date moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RescheduleField {
    /// The planned day (`scheduled_for`).
    ScheduledFor,
    /// The due date (`deadline`).
    Deadline,
}

impl RescheduleField {
    /// Canonical storage string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ScheduledFor => "scheduled_for",
            Self::Deadline => "deadline",
        }
    }

    /// Parse the canonical storage string.
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "scheduled_for" => Some(Self::ScheduledFor),
            "deadline" => Some(Self::Deadline),
            _ => None,
        }
    }
}

/// One recorded move of a task date to a later day.
#[derive(Clone, Debug)]
pub struct TaskReschedule {
    /// Record id (`UUIDv7`).
    pub id: Uuid,
    /// The task that moved.
    pub task_id: Uuid,
    /// Which date moved.
    pub field: RescheduleField,
    /// Previous slot as stored on the task.
    pub from_slot: Option<String>,
    /// New slot as stored on the task.
    pub to_slot: Option<String>,
    /// When the move was made. Moves saved together share this value.
    pub created_at: OffsetDateTime,
}

/// Calendar date of a stored task slot: `YYYY-MM-DD`, optionally followed
/// by `T…` (e.g. `YYYY-MM-DDTHH:MM`). `None` if malformed.
#[must_use]
pub fn slot_date(slot: &str) -> Option<Date> {
    let (date, rest) = slot.split_at_checked(10)?;
    if !(rest.is_empty() || rest.starts_with('T')) {
        return None;
    }
    let mut parts = date.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let month = Month::try_from(m.parse::<u8>().ok()?).ok()?;
    Date::from_calendar_date(y.parse().ok()?, month, d.parse().ok()?).ok()
}

/// Whether changing a slot from `from` to `to` moves it to a later day.
///
/// Only a move between two valid dates counts. Setting a date for the
/// first time, clearing it, moving it earlier, changing only the time of
/// day, or malformed values are not later moves.
#[must_use]
pub fn is_later_move(from: Option<&str>, to: Option<&str>) -> bool {
    match (from.and_then(slot_date), to.and_then(slot_date)) {
        (Some(f), Some(t)) => t > f,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_moves() {
        for (from, to, want) in [
            (Some("2026-06-01"), Some("2026-06-02"), true),
            (Some("2026-06-01T09:00"), Some("2026-06-03"), true),
            (Some("2026-12-31"), Some("2027-01-01"), true),
            (Some("2026-06-02"), Some("2026-06-01"), false),
            (Some("2026-06-01"), Some("2026-06-01T17:00"), false),
            (Some("2026-06-01T09:00"), Some("2026-06-01T17:00"), false),
            (None, Some("2026-06-01"), false),
            (Some("2026-06-01"), None, false),
            (Some("garbage"), Some("2026-06-01"), false),
            (Some("2026-06-01"), Some("2026-13-01"), false),
            (Some("2026-06-01"), Some("2026-06-02x"), false),
        ] {
            assert_eq!(is_later_move(from, to), want, "{from:?} -> {to:?}");
        }
    }

    #[test]
    fn slot_dates() {
        assert_eq!(
            slot_date("2026-02-28T23:59"),
            Date::from_calendar_date(2026, Month::February, 28).ok()
        );
        assert_eq!(slot_date("2026-02-30"), None);
        assert_eq!(slot_date("2026-2-01"), None);
        assert_eq!(slot_date(""), None);
    }

    #[test]
    fn field_round_trips() {
        for f in [RescheduleField::ScheduledFor, RescheduleField::Deadline] {
            assert_eq!(RescheduleField::from_str_opt(f.as_str()), Some(f));
        }
    }
}
