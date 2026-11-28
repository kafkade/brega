//! Repository layer — typed CRUD over `SQLite` tables.

use brega_core::Error as CoreError;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::Error;

pub mod activity_repo;
pub mod annotation_repo;
pub mod area_repo;
pub mod caldav_link_repo;
pub mod checklist_repo;
pub mod context_repo;
pub mod focus_repo;
pub mod habit_repo;
pub mod heading_repo;
pub mod project_repo;
pub mod report_repo;
pub mod sid_repo;
pub mod tag_repo;
pub mod task_repo;
pub mod time_block_repo;
pub mod uda_repo;

pub(crate) const fn invalid_encoding() -> Error {
    Error::Core(CoreError::InvalidEncoding)
}

/// Fixed-width RFC 3339 in UTC with nanosecond precision, so stored
/// timestamps sort chronologically as text (`ORDER BY` on the column).
/// Plain `Rfc3339` trims trailing fractional zeros, which makes the
/// string length vary and `"…20.12Z"` sort after `"…20.1200034Z"`.
const TIMESTAMP_FORMAT: &[time::format_description::BorrowedFormatItem<'static>] = time::macros::format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:9]Z"
);

pub(crate) fn format_timestamp(timestamp: OffsetDateTime) -> Result<String, Error> {
    timestamp
        .to_offset(time::UtcOffset::UTC)
        .format(TIMESTAMP_FORMAT)
        .map_err(|_| invalid_encoding())
}

pub(crate) fn parse_timestamp(raw: &str) -> Result<OffsetDateTime, Error> {
    OffsetDateTime::parse(raw, &Rfc3339).map_err(|_| invalid_encoding())
}

pub(crate) fn parse_optional_timestamp(raw: Option<&str>) -> Result<Option<OffsetDateTime>, Error> {
    raw.map(parse_timestamp).transpose()
}

pub(crate) fn parse_uuid_blob(raw: &[u8]) -> Result<Uuid, Error> {
    Uuid::from_slice(raw).map_err(|_| invalid_encoding())
}

pub(crate) fn parse_optional_uuid_blob(raw: Option<&[u8]>) -> Result<Option<Uuid>, Error> {
    raw.map(parse_uuid_blob).transpose()
}

pub(crate) fn uuid_to_blob(id: Uuid) -> Vec<u8> {
    id.as_bytes().to_vec()
}

pub(crate) const fn bool_to_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub(crate) const fn parse_bool(value: i64) -> Result<bool, Error> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid_encoding()),
    }
}

pub(crate) fn parse_u32(value: i64) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| invalid_encoding())
}

#[cfg(test)]
mod timestamp_tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use time::{Duration, UtcOffset};

    #[test]
    fn stored_timestamps_sort_chronologically_as_text() {
        let a = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
            + Duration::milliseconds(120);
        let b = a + Duration::nanoseconds(3_400);
        let (sa, sb) = (format_timestamp(a).unwrap(), format_timestamp(b).unwrap());
        assert!(sa < sb, "{sa} should sort before {sb}");
        assert_eq!(parse_timestamp(&sa).unwrap(), a);
    }

    #[test]
    fn non_utc_offsets_are_stored_as_utc() {
        let local = OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .unwrap()
            .to_offset(UtcOffset::from_hms(-6, 0, 0).unwrap());
        let s = format_timestamp(local).unwrap();
        assert_eq!(s, "2023-11-14T22:13:20.000000000Z");
        assert_eq!(parse_timestamp(&s).unwrap(), local);
    }
}
