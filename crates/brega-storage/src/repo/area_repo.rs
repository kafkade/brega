//! Repository functions for areas.

use rusqlite::{Connection, Row, params};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{format_timestamp, parse_optional_timestamp, parse_uuid_blob, uuid_to_blob};
use crate::Error;
use brega_core::domain::area::{Area, AreaPatch, NewArea};

const SELECT_AREA_SQL: &str = "SELECT id, name, color, sort_order, weekly_budget_minutes, created_at, modified_at, archived_at FROM areas";

/// Insert a new area row and return it.
///
/// # Errors
/// Returns [`crate::Error::Sqlite`] on database failures and
/// [`crate::Error::Core`] if stored UUID or timestamp data is invalid.
pub fn insert(conn: &Connection, input: &NewArea) -> Result<Area, Error> {
    let id = Uuid::now_v7();
    let created_at = OffsetDateTime::now_utc();
    let created_at_text = format_timestamp(created_at)?;
    validate_budget(input.weekly_budget_minutes)?;

    conn.execute(
        "INSERT INTO areas (id, name, color, sort_order, weekly_budget_minutes, archived_at,
                            created_at, modified_at)
         VALUES (?1, ?2, ?3, 0, ?4, NULL, ?5, ?6)",
        params![
            uuid_to_blob(id),
            input.name,
            input.color,
            input.weekly_budget_minutes,
            created_at_text,
            created_at_text,
        ],
    )?;

    get_by_id(conn, id)?.ok_or(Error::NotFound)
}

/// Apply a patch to an area and return the updated row.
///
/// # Errors
/// [`crate::Error::NotFound`] if the area does not exist,
/// [`crate::Error::InvalidState`] for a zero budget, and
/// [`crate::Error::Sqlite`] on write failures.
pub fn update(conn: &Connection, id: Uuid, patch: &AreaPatch) -> Result<Area, Error> {
    let existing = get_by_id(conn, id)?.ok_or(Error::NotFound)?;
    let name = patch.name.clone().unwrap_or(existing.name);
    let color = patch.color.clone().unwrap_or(existing.color);
    let budget = patch
        .weekly_budget_minutes
        .unwrap_or(existing.weekly_budget_minutes);
    validate_budget(budget)?;
    let now_text = format_timestamp(OffsetDateTime::now_utc())?;
    conn.execute(
        "UPDATE areas SET name = ?1, color = ?2, weekly_budget_minutes = ?3, modified_at = ?4
         WHERE id = ?5",
        params![name, color, budget, now_text, uuid_to_blob(id)],
    )?;
    get_by_id(conn, id)?.ok_or(Error::NotFound)
}

/// List stored areas.
///
/// Archived areas are excluded unless `include_archived` is `true`.
///
/// # Errors
/// Returns [`crate::Error::Sqlite`] on query failures and
/// [`crate::Error::Core`] if stored UUID or timestamp data is invalid.
pub fn list(conn: &Connection, include_archived: bool) -> Result<Vec<Area>, Error> {
    let sql = if include_archived {
        format!("{SELECT_AREA_SQL} ORDER BY sort_order ASC, name ASC")
    } else {
        format!("{SELECT_AREA_SQL} WHERE archived_at IS NULL ORDER BY sort_order ASC, name ASC")
    };

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![])?;
    let mut areas = Vec::new();
    while let Some(row) = rows.next()? {
        areas.push(read_area_row(row)?);
    }
    Ok(areas)
}

/// Archive an area by UUID.
///
/// # Errors
/// Returns [`crate::Error::Sqlite`] on write failures,
/// [`crate::Error::NotFound`] if the area does not exist, and
/// [`crate::Error::Core`] if the timestamp cannot be formatted.
pub fn archive(conn: &Connection, id: Uuid) -> Result<(), Error> {
    let now_text = format_timestamp(OffsetDateTime::now_utc())?;
    let rows_affected = conn.execute(
        "UPDATE areas SET archived_at = ?1, modified_at = ?2 WHERE id = ?3",
        params![now_text, now_text, uuid_to_blob(id)],
    )?;

    if rows_affected == 0 {
        return Err(Error::NotFound);
    }

    Ok(())
}

/// Fetch one area by UUID (archived areas included).
///
/// # Errors
/// [`crate::Error::Sqlite`] on query failures and [`crate::Error::Core`]
/// if stored data cannot be decoded.
pub fn get_by_id(conn: &Connection, id: Uuid) -> Result<Option<Area>, Error> {
    let sql = format!("{SELECT_AREA_SQL} WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![uuid_to_blob(id)])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(read_area_row(row)?));
    }
    Ok(None)
}

fn validate_budget(budget: Option<u32>) -> Result<(), Error> {
    if budget == Some(0) {
        return Err(Error::InvalidState(
            "weekly budget must be greater than zero",
        ));
    }
    Ok(())
}

fn read_area_row(row: &Row<'_>) -> Result<Area, Error> {
    let id_bytes: Vec<u8> = row.get("id")?;
    Ok(Area {
        id: parse_uuid_blob(&id_bytes)?,
        name: row.get("name")?,
        color: row.get("color")?,
        sort_order: row.get("sort_order")?,
        weekly_budget_minutes: row.get("weekly_budget_minutes")?,
        created_at: super::parse_timestamp(&row.get::<_, String>("created_at")?)?,
        modified_at: super::parse_timestamp(&row.get::<_, String>("modified_at")?)?,
        archived_at: parse_optional_timestamp(
            row.get::<_, Option<String>>("archived_at")?.as_deref(),
        )?,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::migrations;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().expect("open");
        migrations::migrate(&mut c).expect("migrate");
        c
    }

    #[test]
    fn budget_is_optional_settable_and_clearable() {
        let c = conn();
        let area = insert(
            &c,
            &NewArea {
                name: "work".into(),
                color: None,
                weekly_budget_minutes: None,
            },
        )
        .expect("insert");
        assert_eq!(area.weekly_budget_minutes, None, "unset means no budget");

        let set = update(
            &c,
            area.id,
            &AreaPatch {
                weekly_budget_minutes: Some(Some(600)),
                ..AreaPatch::default()
            },
        )
        .expect("set");
        assert_eq!(set.weekly_budget_minutes, Some(600));
        assert_eq!(set.name, "work", "untouched fields keep their value");

        let renamed = update(
            &c,
            area.id,
            &AreaPatch {
                name: Some("job".into()),
                ..AreaPatch::default()
            },
        )
        .expect("rename");
        assert_eq!(renamed.weekly_budget_minutes, Some(600));

        let cleared = update(
            &c,
            area.id,
            &AreaPatch {
                weekly_budget_minutes: Some(None),
                ..AreaPatch::default()
            },
        )
        .expect("clear");
        assert_eq!(cleared.weekly_budget_minutes, None);
    }

    #[test]
    fn zero_budget_and_missing_area_are_rejected() {
        let c = conn();
        assert!(matches!(
            insert(
                &c,
                &NewArea {
                    name: "x".into(),
                    color: None,
                    weekly_budget_minutes: Some(0),
                },
            ),
            Err(Error::InvalidState(_))
        ));
        assert!(matches!(
            update(&c, Uuid::now_v7(), &AreaPatch::default()),
            Err(Error::NotFound)
        ));
    }
}
