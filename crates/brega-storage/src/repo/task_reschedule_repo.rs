//! Repository for the append-only task reschedule log (migration 0018).
//!
//! Rows are written by [`super::task_repo::update`] and by sync; the app
//! never updates or deletes them.

use rusqlite::{Connection, Row, params};
use uuid::Uuid;

use super::{format_timestamp, parse_timestamp, parse_uuid_blob, uuid_to_blob};
use crate::Error;
use brega_core::domain::reschedule::{RescheduleField, TaskReschedule};

const SELECT_SQL: &str =
    "SELECT id, task_id, field, from_slot, to_slot, created_at FROM task_reschedules";

/// Append one reschedule record.
///
/// # Errors
/// [`crate::Error::Sqlite`] on write failure.
pub fn insert(conn: &Connection, r: &TaskReschedule) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO task_reschedules (id, task_id, field, from_slot, to_slot, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            uuid_to_blob(r.id),
            uuid_to_blob(r.task_id),
            r.field.as_str(),
            r.from_slot,
            r.to_slot,
            format_timestamp(r.created_at)?,
        ],
    )?;
    Ok(())
}

/// Every reschedule record, oldest first.
///
/// # Errors
/// [`crate::Error::Sqlite`] on query failure, [`crate::Error::Core`] if a
/// stored value cannot be decoded.
pub fn list_all(conn: &Connection) -> Result<Vec<TaskReschedule>, Error> {
    query(conn, &format!("{SELECT_SQL} ORDER BY created_at, id"), &[])
}

/// Reschedule records for one task, oldest first.
///
/// # Errors
/// As [`list_all`].
pub fn list_for_task(conn: &Connection, task_id: Uuid) -> Result<Vec<TaskReschedule>, Error> {
    query(
        conn,
        &format!("{SELECT_SQL} WHERE task_id = ?1 ORDER BY created_at, id"),
        &[&uuid_to_blob(task_id)],
    )
}

fn query(
    conn: &Connection,
    sql: &str,
    args: &[&dyn rusqlite::ToSql],
) -> Result<Vec<TaskReschedule>, Error> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(args)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(read_row(row)?);
    }
    Ok(out)
}

fn read_row(row: &Row<'_>) -> Result<TaskReschedule, Error> {
    let field: String = row.get("field")?;
    Ok(TaskReschedule {
        id: parse_uuid_blob(&row.get::<_, Vec<u8>>("id")?)?,
        task_id: parse_uuid_blob(&row.get::<_, Vec<u8>>("task_id")?)?,
        field: RescheduleField::from_str_opt(&field).ok_or(super::invalid_encoding())?,
        from_slot: row.get("from_slot")?,
        to_slot: row.get("to_slot")?,
        created_at: parse_timestamp(&row.get::<_, String>("created_at")?)?,
    })
}
