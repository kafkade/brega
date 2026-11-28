//! Embedded SQL migrations and the runner that applies them.
//!
//! Migrations are numbered, embedded via `include_str!`, and stored in
//! `schema_migrations` with a sha-256 checksum of the migration SQL.
//! On open, the runner re-hashes each already-applied migration's SQL
//! and refuses to start if any differ — this is a **developer/schema
//! consistency** check, not a security control (an attacker with
//! binary modification capability can change both the SQL and the
//! checksum verification logic).

use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::Error;

/// One embedded migration.
pub struct Migration {
    /// Monotonic version (matches `PRAGMA user_version` after apply).
    pub version: u32,
    /// Human-readable name (also stored for diagnostics).
    pub name: &'static str,
    /// The SQL text to execute.
    pub sql: &'static str,
}

/// All migrations bundled with this build, in apply order.
pub const ALL: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "event_log",
        sql: include_str!("migrations/0002_event_log.sql"),
    },
    Migration {
        version: 3,
        name: "domain_tables",
        sql: include_str!("migrations/0003_domain_tables.sql"),
    },
    Migration {
        version: 4,
        name: "time_blocks",
        sql: include_str!("migrations/0004_time_blocks.sql"),
    },
    Migration {
        version: 5,
        name: "focus_sessions",
        sql: include_str!("migrations/0005_focus_sessions.sql"),
    },
    Migration {
        version: 6,
        name: "habits",
        sql: include_str!("migrations/0006_habits.sql"),
    },
    Migration {
        version: 7,
        name: "uda_definitions",
        sql: include_str!("migrations/0007_uda_definitions.sql"),
    },
    Migration {
        version: 8,
        name: "saved_reports",
        sql: include_str!("migrations/0008_saved_reports.sql"),
    },
    Migration {
        version: 9,
        name: "contexts",
        sql: include_str!("migrations/0009_contexts.sql"),
    },
    Migration {
        version: 10,
        name: "caldav_links",
        sql: include_str!("migrations/0010_caldav_links.sql"),
    },
    Migration {
        version: 11,
        name: "sync_state",
        sql: include_str!("migrations/0011_sync_state.sql"),
    },
    Migration {
        version: 12,
        name: "device_revocation",
        sql: include_str!("migrations/0012_device_revocation.sql"),
    },
    Migration {
        version: 13,
        name: "undo_log",
        sql: include_str!("migrations/0013_undo_log.sql"),
    },
    Migration {
        version: 14,
        name: "checklist_items",
        sql: include_str!("migrations/0014_checklist_items.sql"),
    },
    Migration {
        version: 15,
        name: "app_state",
        sql: include_str!("migrations/0015_app_state.sql"),
    },
    Migration {
        version: 16,
        name: "task_estimate",
        sql: include_str!("migrations/0016_task_estimate.sql"),
    },
    Migration {
        version: 17,
        name: "area_budget",
        sql: include_str!("migrations/0017_area_budget.sql"),
    },
    Migration {
        version: 18,
        name: "task_reschedules",
        sql: include_str!("migrations/0018_task_reschedules.sql"),
    },
];

/// Compute the sha-256 hex digest of `sql`.
#[must_use]
pub fn checksum(sql: &str) -> String {
    use std::fmt::Write as _;
    let mut h = Sha256::new();
    h.update(sql.as_bytes());
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

/// Apply any missing migrations to `conn`.
///
/// The first migration is responsible for creating the
/// `schema_migrations` table itself, so the runner queries
/// `PRAGMA user_version` first (which is `0` on a fresh database) to
/// decide whether the table exists.
///
/// # Errors
/// - [`Error::Sqlite`] for any SQL failure.
/// - [`Error::MigrationChecksumMismatch`] if a recorded checksum no
///   longer matches the embedded SQL.
pub fn migrate(conn: &mut Connection) -> Result<u32, Error> {
    let current: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);

    // Verify checksums of already-applied migrations (only after the
    // schema_migrations table exists, i.e. after migration 1).
    if current >= 1 {
        for m in ALL.iter().take_while(|m| m.version <= current) {
            let stored: Option<String> = conn
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    params![m.version],
                    |r| r.get(0),
                )
                .ok();
            if let Some(stored) = stored
                && stored != checksum(m.sql)
            {
                return Err(Error::MigrationChecksumMismatch { version: m.version });
            }
        }
    }

    // Sync bookkeeping exists from migration 11 on. Snapshot the synced
    // tables' columns before upgrading so the next sync can fill columns
    // this upgrade adds from values other devices already sent (R-13).
    if current >= 11 && ALL.iter().any(|m| m.version > current) {
        crate::sync::record_column_baseline(conn)?;
    }

    for m in ALL {
        if m.version <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        let ts = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default();
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at, checksum)
             VALUES (?1, ?2, ?3, ?4)",
            params![m.version, m.name, ts, checksum(m.sql)],
        )?;
        // PRAGMA user_version takes an integer literal, not a bind parameter.
        tx.execute_batch(&format!("PRAGMA user_version = {};", m.version))?;
        tx.commit()?;
    }

    let final_v: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    Ok(final_v)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// Apply migrations up to and including `version`, as an older build would.
    fn migrate_to(conn: &mut Connection, version: u32) {
        for m in ALL.iter().take_while(|m| m.version <= version) {
            let tx = conn.transaction().expect("tx");
            tx.execute_batch(m.sql).expect("apply");
            tx.execute(
                "INSERT INTO schema_migrations (version, name, applied_at, checksum)
                 VALUES (?1, ?2, '2026-01-01T00:00:00Z', ?3)",
                params![m.version, m.name, checksum(m.sql)],
            )
            .expect("record");
            tx.execute_batch(&format!("PRAGMA user_version = {};", m.version))
                .expect("version");
            tx.commit().expect("commit");
        }
    }

    fn dump(conn: &Connection, sql: &str) -> Vec<Vec<rusqlite::types::Value>> {
        let mut stmt = conn.prepare(sql).expect("prepare");
        let n = stmt.column_count();
        stmt.query_map([], |r| {
            (0..n)
                .map(|i| r.get::<_, rusqlite::types::Value>(i))
                .collect()
        })
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
    }

    #[test]
    fn planning_migrations_keep_existing_rows_unchanged() {
        let mut conn = Connection::open_in_memory().expect("open");
        migrate_to(&mut conn, 15);
        conn.execute_batch(
            "INSERT INTO areas (id, name, sort_order, created_at, modified_at)
             VALUES (x'01', 'home', 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO tasks (id, sid, title, status, area_id, scheduled_for, udas,
                                urgency_cache, created_at, modified_at)
             VALUES (x'02', 1, 'fix sink', 'pending', x'01', '2026-06-01', '{}', 1.5,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
        )
        .expect("seed v15");
        let tasks_before = dump(&conn, "SELECT * FROM tasks");
        let areas_before = dump(&conn, "SELECT * FROM areas");

        assert_eq!(migrate(&mut conn).expect("migrate"), 18);

        let tasks_after = dump(
            &conn,
            "SELECT * FROM tasks", // new column is appended last
        );
        assert_eq!(tasks_after[0][..tasks_before[0].len()], tasks_before[0][..]);
        assert_eq!(
            tasks_after[0].last(),
            Some(&rusqlite::types::Value::Null),
            "estimate starts unset"
        );
        let areas_after = dump(&conn, "SELECT * FROM areas");
        assert_eq!(areas_after[0][..areas_before[0].len()], areas_before[0][..]);
        assert_eq!(areas_after[0].last(), Some(&rusqlite::types::Value::Null));
        assert!(dump(&conn, "SELECT * FROM task_reschedules").is_empty());
    }

    #[test]
    fn planning_columns_reject_non_positive_values() {
        let mut conn = Connection::open_in_memory().expect("open");
        migrate(&mut conn).expect("migrate");
        for sql in [
            "INSERT INTO areas (id, name, sort_order, weekly_budget_minutes, created_at, modified_at)
             VALUES (x'01', 'a', 0, 0, 't', 't')",
            "INSERT INTO tasks (id, sid, title, status, udas, urgency_cache, estimate_minutes,
                                created_at, modified_at)
             VALUES (x'02', 1, 't', 'inbox', '{}', 0, -5, 't', 't')",
        ] {
            assert!(conn.execute_batch(sql).is_err(), "{sql}");
        }
    }

    #[test]
    fn upgrading_records_the_pre_upgrade_column_baseline() {
        let mut conn = Connection::open_in_memory().expect("open");
        migrate_to(&mut conn, 15);
        migrate(&mut conn).expect("migrate");
        let tasks: String = conn
            .query_row(
                "SELECT value FROM sync_state WHERE key = 'known_columns:tasks'",
                [],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .map(|v| String::from_utf8(v).expect("utf8"))
            .expect("baseline recorded");
        assert!(tasks.split(',').any(|c| c == "title"));
        assert!(
            !tasks.split(',').any(|c| c == "estimate_minutes"),
            "baseline is the schema before this upgrade"
        );
    }
}
