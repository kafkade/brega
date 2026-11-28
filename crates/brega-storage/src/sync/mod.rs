//! Sync-time state-diff substrate.
//!
//! Domain repositories write directly to `SQLite` and never emit events.
//! This module bridges that gap at sync time: it synthesizes signed,
//! AEAD-encrypted events by diffing the current domain state against a
//! per-entity journal ([`collect_local_changes`]), and materializes
//! remote events back into the domain tables after running them through
//! the [`brega_sync`] conflict engine ([`ingest_events`]).
//!
//! See `docs/adr/003` (event-sourced sync). All wire payloads are
//! ciphertext; the journal/heads/clock bookkeeping lives in the tables
//! created by migration `0011_sync_state.sql`.

mod registry;
mod row;
mod state;

use std::collections::{HashMap, HashSet};

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use brega_core::event::{DeviceId, EntityKind, EventOp, SignedEvent, VectorClock, verify};
use brega_crypto::signature::VerifyingKey;
use brega_sync::conflict::ConflictPolicy;
use brega_sync::engine::{IngestAction, process_incoming_event};
use uuid::Uuid;

use crate::Error;
use crate::event_log::{self, EventLog};
use crate::vault::OpenVault;

pub use state::{SyncConflict, list_conflicts, resolve_conflict};

// ── Public configuration accessors ───────────────────────────────────

/// `sync_state` key for the configured server base URL.
const KEY_SERVER_URL: &str = "server_url";
/// `sync_state` key for this device's human label.
const KEY_DEVICE_LABEL: &str = "device_label";
/// `sync_state` key for the server pull cursor (opaque monotonic position).
const KEY_PULL_CURSOR: &str = "pull_cursor";
/// `sync_state` key for the explicit server-binding state (ADR-016 §3).
const KEY_BINDING_STATE: &str = "binding_state";
/// `sync_state` key for the server principal **B** — the server-assigned
/// account id returned at registration (ADR-016 §1). Distinct from the
/// client-minted crypto `account_id` (**A**) and the `vault_id` (**V**).
const KEY_SERVER_PRINCIPAL: &str = "server_principal";
/// `sync_state` key for the account email bound at adopt (ADR-016 §9).
const KEY_ACCOUNT_EMAIL: &str = "account_email";
/// `sync_state` key for the one-shot "reconcile before push" flag set at a
/// clone restore (ADR-018 §3B step 3). While set, the next `sync` MUST pull
/// and ingest remote history *before* it pushes any local changes, so a stale
/// clone can never shove old state ahead of newer remote history.
const KEY_PENDING_RECONCILE: &str = "pending_reconcile";

/// A vault has no server binding: **A** and **V** exist, but there is no
/// server URL, principal **B**, or credentials. This is the default after
/// `init` (ADR-016 §3).
pub const BINDING_LOCAL_ONLY: &str = "local_only";
/// A vault bound to exactly one server: the client has persisted the server
/// URL, principal **B**, and credentials (ADR-016 §3).
pub const BINDING_SERVER_BACKED: &str = "server_backed";

/// Columns excluded from an `Update` event's changed-field list (but
/// still carried in the snapshot payload). These are derived metadata
/// timestamps that change on every edit; including them would make every
/// pair of concurrent edits look field-overlapping and force a
/// last-write-wins resolution instead of a clean field merge.
const MERGE_IGNORE_FIELDS: &[&str] = &["modified_at"];

/// Read the persisted sync server URL.
///
/// # Errors
/// [`Error::Sqlite`] on query failure.
pub fn server_url(vault: &OpenVault) -> Result<Option<String>, Error> {
    state::get_state_str(vault.connection(), KEY_SERVER_URL)
}

/// Persist the sync server URL.
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn set_server_url(vault: &OpenVault, url: &str) -> Result<(), Error> {
    state::set_state_str(vault.connection(), KEY_SERVER_URL, url)
}

/// Read this device's human label.
///
/// # Errors
/// [`Error::Sqlite`] on query failure.
pub fn device_label(vault: &OpenVault) -> Result<Option<String>, Error> {
    state::get_state_str(vault.connection(), KEY_DEVICE_LABEL)
}

/// Persist this device's human label.
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn set_device_label(vault: &OpenVault, label: &str) -> Result<(), Error> {
    state::set_state_str(vault.connection(), KEY_DEVICE_LABEL, label)
}

/// Read the server pull cursor (0 if never synced).
///
/// # Errors
/// [`Error::Sqlite`] on query failure.
pub fn pull_cursor(vault: &OpenVault) -> Result<u64, Error> {
    state::get_cursor(vault.connection(), KEY_PULL_CURSOR)
}

/// Persist the server pull cursor.
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn set_pull_cursor(vault: &OpenVault, cursor: u64) -> Result<(), Error> {
    state::set_cursor(vault.connection(), KEY_PULL_CURSOR, cursor)
}

/// Read the explicit binding state, defaulting to [`BINDING_LOCAL_ONLY`].
///
/// The binding state is explicit and recorded at adopt (ADR-016 §3) — it is
/// **not** inferred from the `"local"` device label.
///
/// # Errors
/// [`Error::Sqlite`] on query failure.
pub fn binding_state(vault: &OpenVault) -> Result<String, Error> {
    Ok(state::get_state_str(vault.connection(), KEY_BINDING_STATE)?
        .unwrap_or_else(|| BINDING_LOCAL_ONLY.to_string()))
}

/// Persist the explicit binding state (ADR-016 §3).
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn set_binding_state(vault: &OpenVault, value: &str) -> Result<(), Error> {
    state::set_state_str(vault.connection(), KEY_BINDING_STATE, value)
}

/// Read the persisted server principal **B**, if the vault is server-backed
/// (ADR-016 §1).
///
/// # Errors
/// [`Error::Sqlite`] on query failure.
pub fn server_principal(vault: &OpenVault) -> Result<Option<String>, Error> {
    state::get_state_str(vault.connection(), KEY_SERVER_PRINCIPAL)
}

/// Persist the server principal **B** returned at registration (ADR-016 §1).
/// This is the server-assigned account id; the client previously discarded it.
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn set_server_principal(vault: &OpenVault, principal: &str) -> Result<(), Error> {
    state::set_state_str(vault.connection(), KEY_SERVER_PRINCIPAL, principal)
}

/// Read the account email bound at adopt, if any (ADR-016 §9).
///
/// # Errors
/// [`Error::Sqlite`] on query failure.
pub fn account_email(vault: &OpenVault) -> Result<Option<String>, Error> {
    state::get_state_str(vault.connection(), KEY_ACCOUNT_EMAIL)
}

/// Persist the account email bound at adopt (ADR-016 §9).
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn set_account_email(vault: &OpenVault, email: &str) -> Result<(), Error> {
    state::set_state_str(vault.connection(), KEY_ACCOUNT_EMAIL, email)
}

/// Whether a one-shot reconcile-before-push is pending. Set at a clone
/// restore (ADR-018 §3B) so the next sync pulls remote history before it
/// pushes; cleared once that reconcile has run.
///
/// # Errors
/// [`Error::Sqlite`] on query failure.
pub fn pending_reconcile(vault: &OpenVault) -> Result<bool, Error> {
    Ok(state::get_state_str(vault.connection(), KEY_PENDING_RECONCILE)?.as_deref() == Some("1"))
}

/// Mark that the next sync MUST reconcile (pull + ingest) before pushing
/// (ADR-018 §3B step 3). Idempotent.
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn set_pending_reconcile(vault: &OpenVault) -> Result<(), Error> {
    state::set_state_str(vault.connection(), KEY_PENDING_RECONCILE, "1")
}

/// Clear the one-shot reconcile-before-push flag, once the reconcile pull
/// has completed. Idempotent.
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn clear_pending_reconcile(vault: &OpenVault) -> Result<(), Error> {
    state::delete_state(vault.connection(), KEY_PENDING_RECONCILE)
}

/// Clear the server binding, returning the vault to `LocalOnly`.
///
/// The inverse of adoption (ADR-016 §3 `disconnect`): removes the server URL,
/// principal **B**, email, and binding state, and resets the pull cursor. **A**,
/// **V**, and all local data are preserved — no keys are rotated or re-wrapped.
///
/// # Errors
/// [`Error::Sqlite`] on persistence failure.
pub fn clear_binding(vault: &OpenVault) -> Result<(), Error> {
    let conn = vault.connection();
    for key in [
        KEY_SERVER_URL,
        KEY_SERVER_PRINCIPAL,
        KEY_ACCOUNT_EMAIL,
        KEY_BINDING_STATE,
    ] {
        state::delete_state(conn, key)?;
    }
    state::set_cursor(conn, KEY_PULL_CURSOR, 0)?;
    Ok(())
}

// ── Outbound: collect local changes ──────────────────────────────────

/// Diff every syncable table against the journal and append a signed
/// event for each create / update / delete.
///
/// Updates the journal, head set, and local vector clock, and returns
/// the newly produced events for the caller to push to the server.
///
/// # Errors
/// - [`Error::EventLogIntegrity`] on payload (de)serialization failure.
/// - [`Error::Crypto`] / [`Error::Sqlite`] on append failure.
pub fn collect_local_changes(vault: &OpenVault) -> Result<Vec<SignedEvent>, Error> {
    let conn = vault.connection();
    hydrate_new_columns(conn)?;
    let log = EventLog::new(vault);
    let device = DeviceId::from_bytes(vault.local_device().device_id);
    let mut clock = state::load_vector_clock(conn)?;
    let mut running = log.local_lamport_high()?;
    let mut produced = Vec::new();

    for table in registry::TABLES {
        let local_cols = row::table_columns(conn, table)?;
        let snaps = row::snapshot_table(conn, table)?;
        let journal = state::load_journal(conn, table.kind)?;
        let mut seen: HashSet<Uuid> = HashSet::new();

        for snap in &snaps {
            seen.insert(snap.sync_id);
            match journal.get(&snap.sync_id) {
                None => {
                    let signed = emit(
                        &log,
                        &mut clock,
                        &mut running,
                        device,
                        table.kind,
                        snap.sync_id,
                        EventOp::Create,
                        &snap.content,
                        None,
                    )?;
                    state::put_journal(
                        conn,
                        table.kind,
                        snap.sync_id,
                        &snap.content,
                        signed.event.lamport,
                    )?;
                    state::set_heads(conn, table.kind, snap.sync_id, &[signed.event.id])?;
                    produced.push(signed);
                }
                Some(entry) if entry.content != snap.content => {
                    let Some((content, mut fields)) =
                        diff_against_journal(conn, table.kind, snap, &entry.content, &local_cols)?
                    else {
                        continue;
                    };
                    fields.retain(|f| !MERGE_IGNORE_FIELDS.contains(&f.as_str()));
                    let parent = state::load_heads(conn, table.kind, snap.sync_id)?
                        .first()
                        .copied();
                    let signed = emit(
                        &log,
                        &mut clock,
                        &mut running,
                        device,
                        table.kind,
                        snap.sync_id,
                        EventOp::Update { fields },
                        &content,
                        parent,
                    )?;
                    state::put_journal(
                        conn,
                        table.kind,
                        snap.sync_id,
                        &content,
                        signed.event.lamport,
                    )?;
                    state::set_heads(conn, table.kind, snap.sync_id, &[signed.event.id])?;
                    produced.push(signed);
                }
                Some(_) => {}
            }
        }

        for (sync_id, entry) in &journal {
            if seen.contains(sync_id) {
                continue;
            }
            let cols = row::decode_columns(&entry.content)?;
            let key_cols = row::key_columns_of(table, &cols)?;
            let payload = row::encode_columns(&key_cols);
            let parent = state::load_heads(conn, table.kind, *sync_id)?
                .first()
                .copied();
            let signed = emit(
                &log,
                &mut clock,
                &mut running,
                device,
                table.kind,
                *sync_id,
                EventOp::Delete,
                &payload,
                parent,
            )?;
            state::delete_journal(conn, table.kind, *sync_id)?;
            state::set_heads(conn, table.kind, *sync_id, &[signed.event.id])?;
            produced.push(signed);
        }
    }

    state::store_vector_clock(conn, &clock)?;
    Ok(produced)
}

/// An Update's payload frame and changed field names.
type UpdateDiff = (Vec<u8>, Vec<String>);

/// `sync_state` key prefix for the column set each synced table had when
/// sync last ran.
const KEY_KNOWN_COLUMNS_PREFIX: &str = "known_columns:";

/// Record each synced table's current column set as the hydration baseline,
/// where none is recorded yet.
///
/// [`crate::migrations::migrate`] calls this before applying pending
/// migrations, so the first sync after an upgrade knows which columns the
/// upgrade added.
///
/// # Errors
/// [`Error::Sqlite`] on query or write failure.
pub fn record_column_baseline(conn: &Connection) -> Result<(), Error> {
    for table in registry::TABLES {
        let key = format!("{KEY_KNOWN_COLUMNS_PREFIX}{}", table.table);
        if state::get_state(conn, &key)?.is_none() {
            state::set_state_str(conn, &key, &column_list(conn, table)?)?;
        }
    }
    Ok(())
}

fn column_list(conn: &Connection, table: &registry::SyncTable) -> Result<String, Error> {
    let cols = row::table_columns(conn, table)?;
    let mut sorted: Vec<&str> = cols.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    Ok(sorted.join(","))
}

/// After an upgrade adds columns, fill them from values a newer device
/// already sent, which the journal kept while this build couldn't store
/// them (R-13). Without this, the next collect would see the new column
/// as NULL and emit an Update clearing the value on every device.
///
/// Only columns new since the last sync run are filled, and only where the
/// row is still NULL, so local edits made on this build are never undone.
fn hydrate_new_columns(conn: &Connection) -> Result<(), Error> {
    for table in registry::TABLES {
        let key = format!("{KEY_KNOWN_COLUMNS_PREFIX}{}", table.table);
        let current = column_list(conn, table)?;
        let Some(recorded) = state::get_state_str(conn, &key)? else {
            state::set_state_str(conn, &key, &current)?;
            continue;
        };
        if recorded == current {
            continue;
        }
        let known: HashSet<&str> = recorded.split(',').collect();
        let added: HashSet<&str> = current.split(',').filter(|c| !known.contains(c)).collect();
        if !added.is_empty() {
            for entry in state::load_journal(conn, table.kind)?.values() {
                let cols = row::decode_columns(&entry.content)?;
                let key_cols = row::key_columns_of(table, &cols)?;
                for (name, value) in &cols {
                    if added.contains(name.as_str()) && !matches!(value, Value::Null) {
                        row::fill_null_column(conn, table, &key_cols, name, value)?;
                    }
                }
            }
        }
        state::set_state_str(conn, &key, &current)?;
    }
    Ok(())
}

/// Compare a row whose bytes differ from its journal frame. Returns the
/// Update payload and changed fields, or `None` when the difference is
/// schema-only (see [`reconcile_with_journal`]); in that case the journal is
/// rewritten so later collects take the byte-compare fast path.
fn diff_against_journal(
    conn: &Connection,
    kind: EntityKind,
    snap: &row::RowSnap,
    journal_content: &[u8],
    local_cols: &HashSet<String>,
) -> Result<Option<UpdateDiff>, Error> {
    let old_cols = row::decode_columns(journal_content)?;
    let columns = reconcile_with_journal(&snap.columns, &old_cols, local_cols);
    let content = row::encode_columns(&columns);
    if content != journal_content {
        let fields = row::changed_fields(&old_cols, &columns);
        return Ok(Some((content, fields)));
    }
    let mut full = snap.columns.clone();
    full.extend(
        old_cols
            .into_iter()
            .filter(|(n, _)| !local_cols.contains(n)),
    );
    let full = row::encode_columns(&full);
    if full != journal_content {
        state::rewrite_journal_content(conn, kind, snap.sync_id, &full)?;
    }
    Ok(None)
}

/// Bring a current-row snapshot in line with its last-emitted journal frame
/// so schema differences between builds don't read as edits (R-13):
///
/// - a column absent from the journal frame and NULL now is unchanged (a
///   newly migrated nullable column), so upgrading emits no Update storm;
/// - a column the journal frame carries but the local schema lacks (written
///   by a newer build) is kept, so it round-trips back to newer devices.
fn reconcile_with_journal(
    current: &[(String, Value)],
    journal: &[(String, Value)],
    local_cols: &HashSet<String>,
) -> Vec<(String, Value)> {
    let in_journal = |name: &str| journal.iter().any(|(n, _)| n == name);
    let mut out: Vec<(String, Value)> = current
        .iter()
        .filter(|(n, v)| !matches!(v, Value::Null) || in_journal(n))
        .cloned()
        .collect();
    out.extend(
        journal
            .iter()
            .filter(|(n, _)| !local_cols.contains(n))
            .cloned(),
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn emit(
    log: &EventLog<'_>,
    clock: &mut VectorClock,
    running: &mut u64,
    device: DeviceId,
    kind: EntityKind,
    entity_id: Uuid,
    op: EventOp,
    plaintext: &[u8],
    parent: Option<Uuid>,
) -> Result<SignedEvent, Error> {
    *running += 1;
    clock.0.insert(device, *running);
    log.append(kind, entity_id, op, plaintext, clock.clone(), parent)
}

// ── Inbound: ingest remote events ────────────────────────────────────

/// Outcome of an ingest pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct IngestSummary {
    /// Number of foreign events applied (excludes duplicates already
    /// present locally).
    pub applied: usize,
    /// Number of conflicts recorded for review.
    pub conflicts: usize,
}

/// Verify, decrypt, conflict-resolve, and materialize a batch of remote
/// events into the domain tables.
///
/// Self-events and already-present events are skipped. Updates heads,
/// journal, vector clock, and records any conflicts.
///
/// # Errors
/// - [`Error::EventLogIntegrity`] if an event fails signature or AEAD
///   verification, or its payload cannot be decoded.
/// - [`Error::Sqlite`] on persistence failure.
pub fn ingest_events(vault: &OpenVault, events: &[SignedEvent]) -> Result<IngestSummary, Error> {
    let conn = vault.connection();
    hydrate_new_columns(conn)?;
    let log = EventLog::new(vault);
    let mut clock = state::load_vector_clock(conn)?;
    let mut summary = IngestSummary::default();
    let mut schemas = SchemaCache::default();

    for signed in events {
        if event_log::event_exists(conn, signed.event.id)? {
            continue;
        }
        register_signer(conn, signed)?;
        verify(signed, |id| lookup_key(conn, id)).map_err(|_| Error::EventLogIntegrity)?;
        let plaintext = log.decrypt_payload(signed)?;

        let kind = signed.event.entity_kind;
        let entity_id = signed.event.entity_id;
        let head_ids = state::load_heads(conn, kind, entity_id)?;
        let mut head_events = Vec::with_capacity(head_ids.len());
        for hid in &head_ids {
            if let Some(ev) = event_log::load_event_meta(conn, *hid)? {
                head_events.push(ev);
            }
        }

        let result = process_incoming_event(
            &signed.event,
            &head_events,
            &clock,
            ConflictPolicy::default(),
        );
        clock = result.merged_clock.clone();

        if !matches!(result.action, IngestAction::Discard) {
            materialize(conn, &mut schemas, signed, &plaintext, &result.action)?;
            refresh_journal(conn, &mut schemas, signed, &plaintext)?;
        }

        event_log::insert_event(conn, signed)?;
        state::set_heads(conn, kind, entity_id, &result.new_heads)?;

        for conflict in &result.conflicts {
            let detail = format!(
                "winner={} loser={} resolution={:?}",
                conflict.winning_event_id, conflict.losing_event_id, conflict.resolution
            );
            state::record_conflict(conn, kind, entity_id, &detail)?;
            summary.conflicts += 1;
        }
        summary.applied += 1;
    }

    state::store_vector_clock(conn, &clock)?;
    Ok(summary)
}

/// Per-ingest cache of each table's local column set.
#[derive(Default)]
struct SchemaCache(HashMap<&'static str, HashSet<String>>);

impl SchemaCache {
    fn columns(
        &mut self,
        conn: &Connection,
        table: &'static registry::SyncTable,
    ) -> Result<&HashSet<String>, Error> {
        if !self.0.contains_key(table.table) {
            let cols = row::table_columns(conn, table)?;
            self.0.insert(table.table, cols);
        }
        Ok(&self.0[table.table])
    }
}

fn materialize(
    conn: &Connection,
    schemas: &mut SchemaCache,
    signed: &SignedEvent,
    plaintext: &[u8],
    action: &IngestAction,
) -> Result<(), Error> {
    let Some(table) = registry::table_for(signed.event.entity_kind) else {
        return Ok(());
    };
    match &signed.event.op {
        EventOp::Delete | EventOp::Purge => {
            let key_cols = row::decode_columns(plaintext)?;
            // Tolerate already-absent rows; record but don't abort.
            if let Err(e) = row::delete_row(conn, table, &key_cols) {
                state::record_conflict(
                    conn,
                    signed.event.entity_kind,
                    signed.event.entity_id,
                    &format!("delete failed: {e}"),
                )?;
            }
        }
        EventOp::Create | EventOp::Update { .. } | EventOp::Append { .. } | EventOp::Snapshot => {
            let incoming = row::decode_columns(plaintext)?;
            let merged = match action {
                IngestAction::MergeFields { .. } => merge_fields(conn, table, signed, &incoming)?,
                _ => incoming,
            };
            // Columns from a newer schema are not materialized here; they
            // are carried in the journal frame instead (see refresh_journal).
            let known = schemas.columns(conn, table)?;
            let merged: Vec<(String, Value)> = merged
                .into_iter()
                .filter(|(n, _)| known.contains(n))
                .collect();
            if let Err(e) = row::upsert_row(conn, table, &merged) {
                // Constraint violations (e.g. UNIQUE tag name) become a
                // recorded conflict rather than aborting the whole sync.
                state::record_conflict(
                    conn,
                    signed.event.entity_kind,
                    signed.event.entity_id,
                    &format!("materialize failed: {e}"),
                )?;
            }
        }
    }
    Ok(())
}

/// Field-level merge: overlay only the incoming `Update` fields onto the
/// current local row, preserving disjoint local edits. Falls back to the
/// full incoming snapshot if the row is absent locally.
fn merge_fields(
    conn: &Connection,
    table: &registry::SyncTable,
    signed: &SignedEvent,
    incoming: &[(String, rusqlite::types::Value)],
) -> Result<Vec<(String, rusqlite::types::Value)>, Error> {
    let EventOp::Update { fields } = &signed.event.op else {
        return Ok(incoming.to_vec());
    };
    let key_cols = row::key_columns_of(table, incoming)?;
    let Some(mut current) = row::load_row(conn, table, &key_cols)? else {
        return Ok(incoming.to_vec());
    };
    for field in fields {
        if let Some((_, new_val)) = incoming.iter().find(|(n, _)| n == field) {
            if let Some(slot) = current.iter_mut().find(|(n, _)| n == field) {
                slot.1 = new_val.clone();
            } else {
                current.push((field.clone(), new_val.clone()));
            }
        }
    }
    Ok(current)
}

/// After materializing a remote event, rewrite the journal entry from the
/// now-current row so the next outbound collect does not re-emit it.
/// Columns the local schema lacks are kept in the frame (the incoming
/// event's values over those already journaled) so they round-trip.
fn refresh_journal(
    conn: &Connection,
    schemas: &mut SchemaCache,
    signed: &SignedEvent,
    plaintext: &[u8],
) -> Result<(), Error> {
    let Some(table) = registry::table_for(signed.event.entity_kind) else {
        return Ok(());
    };
    let kind = signed.event.entity_kind;
    let entity_id = signed.event.entity_id;
    match &signed.event.op {
        EventOp::Delete | EventOp::Purge => {
            state::delete_journal(conn, kind, entity_id)?;
        }
        _ => {
            // Re-snapshot via the registry by scanning for the row whose
            // derived sync id matches. For uuid-keyed tables this is the
            // common path; the scan is over a single entity kind.
            let Some(mut columns) = current_columns(conn, table, entity_id)? else {
                return Ok(());
            };
            let known = schemas.columns(conn, table)?;
            let mut extras: Vec<(String, Value)> =
                match state::load_journal_entry(conn, kind, entity_id)? {
                    Some(entry) => row::decode_columns(&entry.content)?
                        .into_iter()
                        .filter(|(n, _)| !known.contains(n))
                        .collect(),
                    None => Vec::new(),
                };
            for (name, value) in row::decode_columns(plaintext)? {
                if known.contains(&name) {
                    continue;
                }
                match extras.iter_mut().find(|(n, _)| *n == name) {
                    Some(slot) => slot.1 = value,
                    None => extras.push((name, value)),
                }
            }
            columns.extend(extras);
            let content = row::encode_columns(&columns);
            state::put_journal(conn, kind, entity_id, &content, signed.event.lamport)?;
        }
    }
    Ok(())
}

/// Find the synced columns of the row whose derived sync id is `sync_id`,
/// by scanning the table's current rows.
fn current_columns(
    conn: &Connection,
    table: &registry::SyncTable,
    sync_id: Uuid,
) -> Result<Option<Vec<(String, Value)>>, Error> {
    for snap in row::snapshot_table(conn, table)? {
        if snap.sync_id == sync_id {
            return Ok(Some(snap.columns));
        }
    }
    Ok(None)
}

fn register_signer(conn: &Connection, signed: &SignedEvent) -> Result<(), Error> {
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default();
    conn.execute(
        "INSERT OR IGNORE INTO devices (device_id, verifying_key, label, registered_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            signed.event.device_id.as_bytes().to_vec(),
            signed.signer.to_bytes().to_vec(),
            Option::<String>::None,
            now,
        ],
    )?;
    Ok(())
}

fn lookup_key(conn: &Connection, id: &DeviceId) -> Option<VerifyingKey> {
    let bytes: Option<Vec<u8>> = conn
        .query_row(
            "SELECT verifying_key FROM devices WHERE device_id = ?1",
            params![id.as_bytes().to_vec()],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    let arr: [u8; 32] = bytes?.as_slice().try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}
